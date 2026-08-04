//! 상태바용 네트워크 정보: 로컬 NIC IPv4(즉시 산출) + 공인 IP(백그라운드 HTTPS 조회).

use std::sync::{Arc, Mutex};

/// 로컬 IP(즉시)와 공인 IP(비동기)를 보관. UI와 백그라운드 스레드가 공유.
#[derive(Clone)]
pub(crate) struct NetInfo {
    pub local: String,
    public: Arc<Mutex<String>>,
}

impl NetInfo {
    /// 로컬 IP를 즉시 구하고, 공인 IP는 백그라운드 스레드로 조회한다(UI 비차단).
    pub(crate) fn new() -> Self {
        let public = Arc::new(Mutex::new("\u{2026}".to_string())); // 조회 전엔 "…".
        let p = public.clone();
        std::thread::spawn(move || {
            let ip = fetch_public_ip().unwrap_or_else(|| "?".to_string());
            if let Ok(mut g) = p.lock() {
                *g = ip;
            }
        });
        Self { local: local_ipv4(), public }
    }

    /// 현재까지 조회된 공인 IP("…"=조회 중, "?"=실패).
    pub(crate) fn public(&self) -> String {
        self.public.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

/// 상태바에 NIC(로컬) IP + 공인 IP를 그린다(각각 클릭하면 클립보드 복사).
pub(crate) fn ip_status(ui: &mut egui::Ui, lang: nabi_i18n::Lang, local: &str, public: &str) {
    use nabi_i18n::tr;
    ui.separator();
    if ui
        .selectable_label(false, format!("\u{1f5a7} {local}"))
        .on_hover_text(tr(lang, "status.localip"))
        .clicked()
    {
        ui.ctx().copy_text(local.to_string());
    }
    if ui
        .selectable_label(false, format!("\u{1f310} {public}"))
        .on_hover_text(tr(lang, "status.publicip"))
        .clicked()
    {
        ui.ctx().copy_text(public.to_string());
    }
}

/// 외부로 라우팅되는 인터페이스의 로컬 IPv4. UDP connect는 실제 패킷을 보내지 않고
/// 커널의 소스 주소 선택만 유도하므로, 그 소켓의 local_addr가 곧 NIC IP다.
pub(crate) fn local_ipv4() -> String {
    let probe = |dst: &str| -> Option<String> {
        let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        s.connect(dst).ok()?;
        Some(s.local_addr().ok()?.ip().to_string())
    };
    probe("8.8.8.8:80").or_else(|| probe("1.1.1.1:80")).unwrap_or_else(|| "?".into())
}

/// 공인 IP를 외부 서비스(api.ipify.org, 평문 IP 응답)에서 HTTPS로 조회한다.
fn fetch_public_ip() -> Option<String> {
    let body = nabi_release::http_get_text("api.ipify.org", "/").ok()?;
    let ip = body.trim();
    // 아주 단순 검증: 비어있지 않고 IPv4/IPv6 길이 범위 + 점/콜론 포함.
    (!ip.is_empty() && ip.len() <= 45 && (ip.contains('.') || ip.contains(':')))
        .then(|| ip.to_string())
}
