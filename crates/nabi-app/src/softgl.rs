//! GPU 없는 VM/헤드리스 대응 — 시작 시 렌더 가능 여부를 사전 프로브하고, 불가하면 소프트웨어
//! OpenGL(Mesa llvmpipe)을 사용자 확인 후 다운로드해 exe 옆에 배치한다(오프라인이면 안내 후
//! 종료). gpu.rs::wgpu_options에 넘길 백엔드(all 또는 GL)를 돌려준다.

use eframe::wgpu::Backends;
use std::path::{Path, PathBuf};

/// Mesa 소프트웨어 GL 자산 zip 내부 파일명 + SHA256(vendor/mesa와 동일).
/// ⚠️ Mesa 버전 갱신 시 vendor/mesa·helppages와 함께 이 해시도 갱신할 것.
const MESA_DLLS: [(&str, &str); 2] = [
    ("opengl32.dll", "12499866437a161d2b250d5105188ae00732dd74b4bebbcdf972e6145af00f9e"),
    ("libgallium_wgl.dll", "1895f8c19ede5efd0497f9dfab463b19bf4377e3af7c06c2d4d073e4680c5f69"),
];
const MESA_ASSET_URL: &str = "https://github.com/matrixism-cmyk/NabiTerm/releases/download/mesa-runtime/nabiTerm-mesa-software-gl.zip";

/// 시작 시 사용할 wgpu 백엔드를 결정한다. 실제 하드웨어 GPU가 있으면 all, 없으면 소프트웨어
/// GL(Mesa llvmpipe)로만 제한, Mesa가 없으면 확인 후 받아 GL. 못 받으면 안내 후 종료한다.
///
/// 핵심 1(크래시 회피): GPU 없는 VM에서 Mesa GL은 기본으로 **d3d12 gallium(WARP 경유)** 을
/// 골라 첫 submission 때 네이티브 `E_INVALIDARG(0x80070057)`로 죽는다. `force_mesa_llvmpipe`로
/// **llvmpipe(순수 CPU)** 를 강제하고, 소프트웨어 경로에서는 `Backends::GL`만 돌려줘 wgpu가
/// DX12-WARP를 고르지 못하게 한다. 하드웨어 판정은 GL을 제외한 DX12/Vulkan 프로브로만 해
/// 크래시 경로(Mesa d3d12)를 건드리지 않는다.
/// 핵심 2(DLL): Mesa는 **항상 쓰기 가능한** `%LOCALAPPDATA%\nabiTerm\mesa`에 저장하고(전체
/// 설치 시 Program Files 쓰기 실패·VirtualStore 회피), GL 로드 전에 SetDllDirectory로 등록한다.
/// 새로 받은 직후엔 이미 system opengl32가 매핑돼 교체 불가 → 깨끗한 프로세스로 1회 재실행.
pub(crate) fn resolve_backends() -> Backends {
    force_mesa_llvmpipe(); // Mesa가 크래시 유발 d3d12 대신 llvmpipe를 쓰도록(벤더 드라이버는 무시).
    let mdir = mesa_dir();
    register_dll_dir(&mdir); // GL 로드보다 먼저: 사용자 Mesa를 시스템 opengl32보다 우선 검색.

    match std::env::var("NABI_RENDERER").ok().as_deref() {
        Some("software" | "gl") => return Backends::GL,
        Some("hardware" | "gpu" | "wgpu") => return Backends::all(),
        _ => {}
    }
    // 실제 GPU(Discrete/Integrated/Virtual)가 있으면 전 백엔드 허용. WARP(Cpu)는 하드웨어 아님.
    if has_hardware_gpu() {
        return Backends::all();
    }
    // 하드웨어 없음 → 소프트웨어 GL. 쓸 수 있는 GL 어댑터(벤더 GL 또는 llvmpipe)가 있으면 GL만.
    if gl_adapter_ok() {
        return Backends::GL;
    }
    // GL 어댑터도 없음(system GL 미지원 + Mesa 미설치). 이미 받은 Mesa가 있으면 재실행 후 GL.
    if mesa_present(&mdir) || mesa_present(&exe_dir()) {
        if !already_restarted() {
            reexec();
        }
        return Backends::GL;
    }
    // 렌더 어댑터가 전혀 없음 → 사용자 확인 후 소프트웨어 GL 받기.
    if !confirm_download() {
        notify_manual("소프트웨어 렌더링 구성요소가 필요합니다.");
        std::process::exit(0);
    }
    match fetch_mesa(&mdir) {
        Ok(()) => {
            // 현재 프로세스엔 이미 system opengl32가 매핑됨 → 새 프로세스에서 Mesa로 시작.
            if !already_restarted() {
                reexec();
            }
            Backends::GL
        }
        Err(e) => {
            notify_manual(&format!("자동 설치 실패: {e}"));
            std::process::exit(0);
        }
    }
}

/// Mesa가 d3d12 gallium(WARP) 대신 llvmpipe를 쓰도록 강제(이미 설정돼 있으면 존중).
/// 둘 다 Mesa 전용 변수라 실하드웨어 벤더 드라이버(NVIDIA/AMD/Intel opengl32)는 무시 → 무해.
fn force_mesa_llvmpipe() {
    for (k, v) in [("GALLIUM_DRIVER", "llvmpipe"), ("LIBGL_ALWAYS_SOFTWARE", "1")] {
        if std::env::var_os(k).is_none() {
            std::env::set_var(k, v);
        }
    }
}

/// Mesa DLL 저장 경로 — 전체 설치(Program Files)에서도 쓰기 가능한 사용자 로컬 폴더.
fn mesa_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("nabiTerm").join("mesa"))
        .unwrap_or_else(|| exe_dir().join("mesa"))
}

fn already_restarted() -> bool {
    std::env::var_os("NABI_GL_RESTARTED").is_some()
}

/// 동일 인자로 자기 자신을 재실행하고 종료한다(가드 env로 1회만).
fn reexec() -> ! {
    if let Ok(exe) = std::env::current_exe() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let _ = std::process::Command::new(exe)
            .args(args)
            .env("NABI_GL_RESTARTED", "1")
            .spawn();
    }
    std::process::exit(0);
}

/// 사용자 Mesa 경로를 DLL 검색 경로에 추가한다(opengl32.dll은 KnownDLL이 아니라 가능).
/// exe 옆(앱 디렉터리)은 항상 먼저 검색되므로 번들 배치는 이와 무관하게 동작한다.
#[cfg(windows)]
fn register_dll_dir(dir: &Path) {
    use windows::core::HSTRING;
    use windows::Win32::System::LibraryLoader::SetDllDirectoryW;
    let _ = std::fs::create_dir_all(dir);
    let h = HSTRING::from(dir.to_string_lossy().as_ref());
    // SAFETY: HSTRING이 NUL로 끝나는 UTF-16 버퍼를 보증하고 호출이 끝날 때까지 살아 있다.
    // SetDllDirectoryW는 문자열을 복사해 보관한다.
    unsafe {
        let _ = SetDllDirectoryW(&h);
    }
}

#[cfg(not(windows))]
fn register_dll_dir(_dir: &Path) {}

/// 지정 백엔드로 어댑터를 요청한다(렌더 없이 정보 조회만 → 크래시 경로를 건드리지 않음).
fn request_adapter(backends: Backends) -> Option<eframe::wgpu::Adapter> {
    use eframe::wgpu;
    // wgpu 29: InstanceDescriptor는 from_env_or_default 기반, Instance::new는 참조를 받고
    // request_adapter는 Result를 돌려준다.
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let inst = wgpu::Instance::new(desc);
    pollster::block_on(inst.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    }))
    .ok()
}

/// 실제 하드웨어 GPU가 있는지 — GL을 제외한 DX12/Vulkan로만 프로브해 Mesa d3d12 크래시 경로를
/// 피한다. WARP는 DeviceType::Cpu로 잡히므로 GPU로 치지 않는다(소프트웨어 경로로 보냄).
fn has_hardware_gpu() -> bool {
    use eframe::wgpu::DeviceType;
    let Some(ad) = request_adapter(Backends::DX12 | Backends::VULKAN) else {
        return false;
    };
    matches!(
        ad.get_info().device_type,
        DeviceType::DiscreteGpu | DeviceType::IntegratedGpu | DeviceType::VirtualGpu
    )
}

/// GL 어댑터(벤더 OpenGL 또는 llvmpipe 강제된 Mesa)를 쓸 수 있는지. llvmpipe가 강제돼 있어
/// 여기서 만들어지는 GL 어댑터는 안전(크래시 유발 d3d12가 아님).
fn gl_adapter_ok() -> bool {
    request_adapter(Backends::GL).is_some()
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default()
}

fn mesa_present(dir: &Path) -> bool {
    MESA_DLLS.iter().all(|(n, _)| dir.join(n).exists())
}

/// "GPU 미감지 → 약 22MB 받을까요?" 네이티브 확인창(Yes/No).
fn confirm_download() -> bool {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title("nabiTerm — 소프트웨어 렌더링")
        .set_description(
            "이 PC에서 GPU가 감지되지 않아 nabiTerm을 그릴 수 없습니다.\n\
             소프트웨어 렌더링 구성요소(Mesa, 약 22MB)를 지금 받을까요?\n\
             (인터넷 연결이 필요합니다.)",
        )
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}

/// 받지 못했을 때(거부·오프라인·실패) 수동 안내(자산 링크) — 호출측이 곧 종료한다.
fn notify_manual(reason: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("nabiTerm")
        .set_description(format!(
            "{reason}\n\n아래에서 nabiTerm-mesa-software-gl.zip 을 받아\n\
             두 DLL(opengl32.dll, libgallium_wgl.dll)을 다음 폴더에 풀어 주세요:\n{}\n\n{}",
            mesa_dir().display(),
            nabi_release::RELEASES_URL,
        ))
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Mesa 자산 zip을 받아 exe 옆에 풀고 SHA256을 검증한다(임시 zip은 정리).
fn fetch_mesa(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = std::env::temp_dir().join("nabiTerm-mesa-software-gl.zip");
    nabi_release::download_file(MESA_ASSET_URL, &tmp.to_string_lossy())?;
    let res = extract_and_verify(&tmp, dir);
    let _ = std::fs::remove_file(&tmp);
    res
}

fn extract_and_verify(zip_path: &Path, dir: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut ar = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    for (name, expected) in MESA_DLLS {
        let dst = dir.join(name);
        {
            let mut entry = ar.by_name(name).map_err(|e| format!("{name}: {e}"))?;
            let mut out = std::fs::File::create(&dst).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
        if sha256_hex(&dst).as_deref() != Some(expected) {
            let _ = std::fs::remove_file(&dst);
            return Err(format!("{name} 무결성 검증 실패(SHA256 불일치)"));
        }
    }
    Ok(())
}

fn sha256_hex(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let data = std::fs::read(path).ok()?;
    let mut h = Sha256::new();
    h.update(&data);
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
