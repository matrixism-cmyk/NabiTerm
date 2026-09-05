//! **무엇으로 화면을 그리고 있는가** — 상태 표시줄에 보여 준다.
//!
//! 같은 코드인데 그래픽 어댑터에 따라 다르게 깨지는 일이 있다. 진짜 GPU 로 그릴 때,
//! 소프트웨어(Mesa)로 물러났을 때, 원격 데스크톱의 가상 어댑터일 때가 저마다 다르다.
//!
//! 그런데 지금 어느 쪽인지 화면 어디에도 나오지 않았다. 화면이 이상하다는 보고를 받아도
//! 무엇으로 그리고 있었는지부터 물어봐야 했다(사용자 요청 2026-09-05).
//!
//! 그래서 시작할 때 한 번 물어 적어 두고, 상태 표시줄에서 보여 준다.

/// 지금 쓰는 어댑터 요약.
#[derive(Clone, Default)]
pub(crate) struct GpuInfo {
    /// 상태 표시줄에 넣을 짧은 이름 — 백엔드 + 어댑터 종류.
    pub short: String,
    /// 마우스를 올렸을 때 보여 줄 자세한 것 — 어댑터 이름·드라이버.
    pub detail: String,
}

/// eframe 이 고른 어댑터를 물어 요약한다. wgpu 가 아니면 비어 있다.
pub(crate) fn probe(cc: &eframe::CreationContext<'_>) -> GpuInfo {
    let Some(rs) = cc.wgpu_render_state.as_ref() else {
        // wgpu 가 아니면(glow 등) 물어볼 곳이 없다 — 빈 값이면 상태바가 안 그린다.
        return GpuInfo::default();
    };
    let info = rs.adapter.get_info();
    let kind = device_kind(info.device_type);
    GpuInfo {
        short: format!("{} \u{00b7} {kind}", backend_name(info.backend)),
        detail: format!(
            "{}\n{} {}\n{}",
            info.name,
            backend_name(info.backend),
            kind,
            match info.driver_info.is_empty() {
                true => info.driver.clone(),
                false => format!("{} {}", info.driver, info.driver_info),
            }
        ),
    }
}

/// 백엔드 이름 — 짧게.
fn backend_name(b: eframe::wgpu::Backend) -> &'static str {
    use eframe::wgpu::Backend;
    match b {
        Backend::Vulkan => "Vulkan",
        Backend::Dx12 => "D3D12",
        Backend::Metal => "Metal",
        Backend::Gl => "OpenGL",
        Backend::BrowserWebGpu => "WebGPU",
        Backend::Noop => "없음",
    }
}

/// 어댑터 종류 — 사람이 읽는 말로.
///
/// "Cpu" 를 그대로 두지 않는다. 소프트웨어로 그리고 있다는 사실이 **문제를 푸는 실마리**인
/// 경우가 많아서, 그 말이 눈에 바로 들어와야 한다.
fn device_kind(t: eframe::wgpu::DeviceType) -> &'static str {
    use eframe::wgpu::DeviceType;
    match t {
        DeviceType::DiscreteGpu => "외장 GPU",
        DeviceType::IntegratedGpu => "내장 GPU",
        DeviceType::VirtualGpu => "가상 GPU",
        DeviceType::Cpu => "소프트웨어",
        DeviceType::Other => "기타",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::wgpu::{Backend, DeviceType};

    #[test]
    fn 이름을_사람_말로_바꾼다() {
        assert_eq!(backend_name(Backend::Dx12), "D3D12");
        assert_eq!(backend_name(Backend::Gl), "OpenGL");
        assert_eq!(device_kind(DeviceType::Cpu), "소프트웨어");
        assert_eq!(device_kind(DeviceType::DiscreteGpu), "외장 GPU");
    }

    /// 빈 값이면 상태바가 아무것도 그리지 않는다 — 그 약속을 시험으로 붙잡는다.
    #[test]
    fn 못_물어보면_비어_있다() {
        assert!(GpuInfo::default().short.is_empty());
    }
}
