//! wgpu 렌더러 설정 — glow(OpenGL) 단일 백엔드는 하이브리드 그래픽·RDP·가상 GPU·구형 인텔
//! 드라이버에서 컨텍스트 생성/스왑이 실패해 화면이 안 보이는 사례가 있어 wgpu로 교체했다.
//! 모든 백엔드(DX12→Vulkan→GL)를 허용하고 통합 GPU를 선호해 어디서나 창이 뜨게 한다.

/// eframe 렌더러용 wgpu 설정. 통합 GPU(LowPower) 선호. 백엔드는 softgl::resolve_backends가
/// 결정한다(하드웨어=all, GPU 없는 VM=GL 소프트웨어 폴백).
pub(crate) fn wgpu_options(backends: eframe::wgpu::Backends) -> eframe::egui_wgpu::WgpuConfiguration {
    eframe::egui_wgpu::WgpuConfiguration {
        supported_backends: backends,
        power_preference: eframe::wgpu::PowerPreference::LowPower,
        ..Default::default()
    }
}
