//! 어떤 그래픽 백엔드를 쓸지 정한다 — **순수 판정**과 크래시 되돌림 표식.
//!
//! ## 왜 Windows에서 Vulkan을 빼는가
//!
//! 사용자 보고(2026-08-22): Intel UHD 620 노트북에서 nabiTerm이 시작하자마자 죽었다.
//! 이벤트 로그의 크래시 10건이 전부 같은 모듈이었다 —
//! `igvk64.dll 31.0.101.2121`(Intel Vulkan ICD), 예외 `0xc0000005`(액세스 위반).
//!
//! 우리 잘못이 두 겹이었다.
//! 1. 하드웨어 GPU가 있는지 **프로브할 때** `DX12 | VULKAN`을 썼다 → 그 순간 ICD가 로드된다.
//! 2. GPU가 있으면 `Backends::all()`을 돌려줬다 → 실제 인스턴스도 Vulkan을 잡는다.
//!
//! 그래서 `WGPU_BACKEND=dx12`로도 피할 수 없었다. 백엔드를 고르기 **전에**, 어댑터를
//! 열거하는 단계에서 이미 깨진 ICD를 로드하기 때문이다(사용자가 직접 확인해 알려 주었다).
//!
//! Windows에서 2D UI에 Vulkan이 주는 이득은 없다 — DX12가 1순위, OpenGL이 폴백이면 충분하다.
//! 그래서 기본에서 아예 뺀다. 필요하면 `NABI_RENDERER=vulkan`으로 되살릴 수 있다.

use eframe::wgpu::Backends;

/// `NABI_RENDERER` 값 → 백엔드. 값이 없으면 `None`(자동 판정으로 넘긴다).
///
/// 표식(`crashed`)이 있으면 **무조건 GL**이다. 지난 실행이 그래픽 초기화 도중에 죽었다는
/// 뜻이라, 같은 선택을 반복하면 영영 못 켠다.
pub(crate) fn backends_for(env: Option<&str>, crashed: bool) -> Option<Backends> {
    if crashed {
        return Some(Backends::GL);
    }
    match env.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("software" | "gl" | "opengl") => Some(Backends::GL),
        Some("dx12" | "d3d12" | "directx") => Some(Backends::DX12),
        // 명시적으로 켤 때만 Vulkan을 허용한다(구형 인텔 ICD가 깨져 있다).
        Some("vulkan" | "vk") => Some(Backends::VULKAN | Backends::DX12 | Backends::GL),
        Some("hardware" | "gpu" | "wgpu") => Some(safe_hardware()),
        _ => None,
    }
}

/// 하드웨어 GPU가 있을 때 쓸 백엔드 집합. Windows에서는 **Vulkan을 뺀다**(위 설명 참조).
pub(crate) fn safe_hardware() -> Backends {
    #[cfg(windows)]
    {
        Backends::DX12 | Backends::GL
    }
    #[cfg(not(windows))]
    {
        // 리눅스·맥에서는 Vulkan/Metal이 정상 경로다. 문제는 Windows의 구형 Intel ICD였다.
        Backends::all()
    }
}

/// 하드웨어가 있는지 프로브할 때 쓸 백엔드. 프로브 자체가 ICD를 로드하므로 여기서도 뺀다.
pub(crate) fn probe_backends() -> Backends {
    #[cfg(windows)]
    {
        Backends::DX12
    }
    #[cfg(not(windows))]
    {
        Backends::VULKAN | Backends::METAL
    }
}

/// "그래픽 초기화 중"이라고 남기는 표식 파일의 경로.
///
/// 시작할 때 만들고, 첫 프레임이 뜨면 지운다. 다음 실행에서 이 파일이 남아 있으면
/// **지난번에 초기화 도중 죽은 것**이므로 안전한 GL로 내려간다. 드라이버가 깨진 기계에서도
/// 프로그램이 영영 안 켜지는 상태로 남지 않게 하는 마지막 방어선이다.
pub(crate) fn marker_path() -> std::path::PathBuf {
    nabi_config::paths::resolve_base().join("gpu-init.flag")
}

/// 표식이 남아 있는가(= 지난 실행이 그래픽 초기화 중 죽었다).
pub(crate) fn crashed_last_time() -> bool {
    marker_path().exists()
}

/// 표식을 남긴다(그래픽 초기화 직전).
///
/// **여기서는 실패를 알리지 않는다**(배치 AF 에서 훑고 판단한 결과). 이 표식은 그래픽
/// 초기화가 죽었을 때 다음 실행이 알아채려고 두는 안전망이다. 못 남기면 안전망 하나를
/// 잃을 뿐 지금 하려는 일은 그대로 되고, 실행할 때마다 "표식을 못 남겼습니다"가 뜨면
/// 정작 중요한 알림까지 읽지 않게 된다. 잃는 것보다 소음이 크다.
pub(crate) fn mark_starting() {
    let p = marker_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&p, b"gpu init in progress");
}

/// 표식을 지운다(첫 프레임이 무사히 떴을 때).
pub(crate) fn mark_ok() {
    let _ = std::fs::remove_file(marker_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 사용자 보고의 핵심 — Windows 기본 경로에 Vulkan이 **절대** 들어가면 안 된다.
    #[test]
    #[cfg(windows)]
    fn windows_never_picks_vulkan_by_default() {
        assert!(!safe_hardware().contains(Backends::VULKAN), "기본에 Vulkan이 있으면 안 된다");
        assert!(!probe_backends().contains(Backends::VULKAN), "프로브도 ICD를 로드한다");
        assert!(safe_hardware().contains(Backends::DX12));
        assert!(safe_hardware().contains(Backends::GL), "DX12가 안 되면 GL로 내려갈 수 있어야 한다");
    }

    #[test]
    fn env_values_map_to_backends() {
        assert_eq!(backends_for(Some("software"), false), Some(Backends::GL));
        assert_eq!(backends_for(Some("GL"), false), Some(Backends::GL));
        assert_eq!(backends_for(Some(" dx12 "), false), Some(Backends::DX12));
        assert_eq!(backends_for(None, false), None, "값이 없으면 자동 판정에 맡긴다");
        assert_eq!(backends_for(Some("nonsense"), false), None);
    }

    /// Vulkan은 **명시적으로 요청할 때만** 켜진다.
    #[test]
    fn vulkan_is_opt_in_only() {
        let v = backends_for(Some("vulkan"), false).unwrap();
        assert!(v.contains(Backends::VULKAN));
        assert!(v.contains(Backends::DX12), "Vulkan을 켜도 폴백은 남긴다");
        assert!(!backends_for(Some("hardware"), false).unwrap().contains(Backends::VULKAN));
    }

    /// 지난 실행이 초기화 중 죽었으면 무엇을 요청했든 GL이다 — 못 켜는 상태로 남지 않게.
    #[test]
    fn a_crashed_start_falls_back_to_gl() {
        for env in [None, Some("vulkan"), Some("hardware"), Some("dx12")] {
            assert_eq!(backends_for(env, true), Some(Backends::GL), "env={env:?}");
        }
    }
}
