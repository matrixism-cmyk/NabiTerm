//! 볼트 마스터 비밀번호의 OS 자격증명 저장(선택, F1) — Windows 자격증명 관리자.
//!
//! 기본은 사용 안 함(opt-in). 저장하면 같은 OS 세션 사용자가 볼트를 자동으로 열 수 있어
//! 편의↑·보안↓의 절충이다. 호출부에서 반드시 사용자 동의(체크박스) 후에만 저장할 것.

const SERVICE: &str = "nabiTerm-vault";
const USER: &str = "master";

fn entry() -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, USER).ok()
}

/// 마스터 비밀번호를 OS 자격증명에 저장한다(성공 시 true).
pub fn store_master(password: &str) -> bool {
    entry().map(|e| e.set_password(password).is_ok()).unwrap_or(false)
}

/// 저장된 마스터 비밀번호를 가져온다(없으면 None).
pub fn load_master() -> Option<String> {
    entry()?.get_password().ok()
}

/// 저장된 마스터 비밀번호를 삭제한다(best-effort).
pub fn clear_master() {
    if let Some(e) = entry() {
        let _ = e.delete_credential();
    }
}

const TG_SERVICE: &str = "nabiTerm-telegram";
const TG_USER: &str = "bot-token";

fn tg_entry() -> Option<keyring::Entry> {
    keyring::Entry::new(TG_SERVICE, TG_USER).ok()
}

/// 텔레그램 봇 토큰을 OS 자격증명에 저장한다(성공 시 true). 설정 평문 금지 — 비밀이라 keyring.
pub fn store_telegram_token(token: &str) -> bool {
    tg_entry().map(|e| e.set_password(token).is_ok()).unwrap_or(false)
}

/// 저장된 텔레그램 봇 토큰을 가져온다(없으면 None).
pub fn load_telegram_token() -> Option<String> {
    tg_entry()?.get_password().ok()
}

/// 저장된 텔레그램 봇 토큰을 삭제한다(best-effort).
pub fn clear_telegram_token() {
    if let Some(e) = tg_entry() {
        let _ = e.delete_credential();
    }
}
