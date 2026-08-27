//! 살아 있는 SSH 연결의 pane별 레지스트리(배치 Y H4) — **SFTP가 다시 붙지 않게 한다.**
//!
//! 지금까지 SSH로 붙은 뒤 SFTP를 열면 연결을 새로 만들고 인증을 다시 했다. 사용자에게는
//! 비밀번호를 두 번 묻는 것으로, 서버에는 세션이 두 개로, 점프 호스트에는 터널 두 벌로
//! 보였다. 접속 수를 제한하는 서버에서는 그 자체가 문제다.
//!
//! ## 왜 새 구조가 아니라 이 모양인가
//!
//! `kexinfo.rs`가 이미 같은 일을 한다 — pane을 열쇠로 한 정적 레지스트리에 `set`/`get`/
//! `clear`, 그리고 **세션이 끝날 때 `clear`를 부르는 자리까지** 있다. 두 번째 레지스트리를
//! 다른 모양으로 만들면 언젠가 한쪽만 정리된다. 그래서 뼈대를 그대로 따랐고, `clear`도
//! `kexinfo::clear`와 **같은 줄에** 둔다.
//!
//! ## 수명 — 누가 연결의 주인인가
//!
//! 레지스트리는 pane이 닫힐 때 지운다. 그런데 **이미 핸들을 받아 간 SFTP는 자기 `Arc`로
//! 계속 산다.** 즉 전송 중에 터미널 탭을 닫아도 그 전송은 끝까지 간다 — 파일을 올리다
//! 탭을 닫았다고 전송이 깨지면 그것이 결함이다. `SftpFs`가 점프 핸들을 붙들어 터널을
//! 유지하는 것과 같은 논리다.
//!
//! 반대로 `clear`를 빠뜨리면 **닫힌 pane의 연결로 새 SFTP가 열린다.** 그래서 `clear`는
//! 선택이 아니다.

use crate::handler::ClientHandler;
use nabi_types::PaneId;
use russh::client::Handle;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// 한 pane이 붙들고 있는 살아 있는 SSH 연결.
///
/// `jump`는 점프 호스트(ProxyJump) 핸들이다. **드롭되면 터널이 끊기므로** 목적지 핸들만
/// 넘겨받으면 안 된다 — 둘을 함께 들고 가야 한다.
#[derive(Clone)]
pub struct SshConn {
    pub handle: Arc<Handle<ClientHandler>>,
    pub jump: Option<Arc<Handle<ClientHandler>>>,
    /// 이 연결이 **누구로 어디에** 붙어 있는지. 재사용 대상을 고를 때 대조한다.
    pub who: Who,
}

/// 연결의 신원 — 재사용해도 되는지 판정하는 유일한 기준.
///
/// 사용자 이름까지 보는 것이 핵심이다. 같은 서버라도 **다른 계정으로 붙은 연결을 물려주면
/// 그 계정의 권한으로 파일을 만지게 된다.** 호스트만 맞다고 재사용하면 그것은 권한 상승이다.
/// 점프 호스트도 대조한다 — 경유지가 다르면 실제로 닿는 곳이 다를 수 있다.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Who {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// 점프 호스트의 `host:port`(없으면 `None`).
    pub jump: Option<(String, u16)>,
}

impl Who {
    /// 접속 정보에서 신원을 뽑는다.
    pub fn of(p: &nabi_proto::SshParams) -> Self {
        Self {
            host: p.host.clone(),
            port: p.port,
            user: p.user.clone(),
            jump: p.jump.as_ref().map(|j| (j.host.clone(), j.port)),
        }
    }
}

impl SshConn {
    /// 연결이 아직 살아 있는가. 죽은 핸들을 건네주면 SFTP가 그것으로 파일을 쓰려 든다.
    pub fn alive(&self) -> bool {
        !self.handle.is_closed()
    }
}

fn registry() -> &'static Mutex<HashMap<PaneId, SshConn>> {
    static REG: OnceLock<Mutex<HashMap<PaneId, SshConn>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// pane의 연결을 등록한다(재연결 시 덮어씀).
pub fn set(pane: PaneId, conn: SshConn) {
    if let Ok(mut m) = registry().lock() {
        m.insert(pane, conn);
    }
}

/// pane의 살아 있는 연결(없거나 이미 끊겼으면 `None`).
///
/// 끊긴 것은 **돌려주지 않고 지운다.** 죽은 핸들을 들고 있는 것보다 새로 연결하는 편이
/// 언제나 낫다 — 사용자는 비밀번호를 한 번 더 칠 뿐이지만, 죽은 핸들은 조용히 실패한다.
pub fn get(pane: PaneId) -> Option<SshConn> {
    let mut m = registry().lock().ok()?;
    match m.get(&pane) {
        Some(c) if c.alive() => Some(c.clone()),
        Some(_) => {
            m.remove(&pane);
            None
        }
        None => None,
    }
}

/// 세션 종료 시 제거. 이미 넘겨준 `Arc`는 받은 쪽이 놓을 때까지 산다.
pub fn clear(pane: PaneId) {
    if let Ok(mut m) = registry().lock() {
        m.remove(&pane);
    }
}

/// 지금 등록된 연결 수(상태 표시줄이 "연결 하나/둘"을 보여 줄 때 쓴다).
pub fn count() -> usize {
    registry().lock().map(|m| m.len()).unwrap_or(0)
}

/// **같은 신원**의 살아 있는 연결을 찾는다 — SFTP가 물려받을 대상.
///
/// pane 번호가 아니라 신원으로 찾는 이유: 메뉴 문구가 이미 "SFTP 열기(**같은 서버**)"다.
/// 사용자는 어느 pane 인지가 아니라 어느 서버인지로 생각한다. pane 을 인자로 끌고 다니면
/// `spawn_sftp` 까지 여러 층을 고쳐야 하는데, 그렇게 얻는 것도 결국 같은 판정이다.
///
/// 끊긴 것은 지나친다(`get`과 달리 여기서는 지우지 않는다 — 소유자 pane 이 정리한다).
pub fn find(who: &Who) -> Option<SshConn> {
    let m = registry().lock().ok()?;
    m.values().find(|c| &c.who == who && c.alive()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 핸들은 실제 연결 없이 만들 수 없다. 그래서 여기서는 레지스트리의 **계약**만 본다 —
    // 없는 pane은 None, clear 후에도 None, count가 따라 움직인다.
    #[test]
    fn empty_pane_has_no_conn() {
        let pane = PaneId::new(u64::MAX - 11); // 실사용과 겹치지 않는 키.
        assert!(get(pane).is_none());
        clear(pane); // 없는 것을 지워도 터지지 않아야 한다.
        assert!(get(pane).is_none());
    }

    fn params(host: &str, port: u16, user: &str) -> nabi_proto::SshParams {
        nabi_proto::SshParams::agent(host, port, user)
    }

    #[test]
    fn identity_needs_user_to_match() {
        // 같은 서버라도 계정이 다르면 다른 연결이다 — 이것을 놓치면 권한 상승이 된다.
        let a = Who::of(&params("h", 22, "alice"));
        let b = Who::of(&params("h", 22, "bob"));
        assert_ne!(a, b);
        assert_eq!(a, Who::of(&params("h", 22, "alice")));
    }

    #[test]
    fn identity_distinguishes_port() {
        assert_ne!(Who::of(&params("h", 22, "u")), Who::of(&params("h", 2222, "u")));
    }

    #[test]
    fn identity_distinguishes_jump_host() {
        // 경유지가 다르면 실제로 닿는 곳이 다를 수 있다.
        let plain = Who::of(&params("h", 22, "u"));
        let mut viajump = Who::of(&params("h", 22, "u"));
        viajump.jump = Some(("gw".into(), 22));
        assert_ne!(plain, viajump);
    }

    #[test]
    fn no_live_conn_means_no_reuse() {
        assert!(find(&Who::of(&params("nonexistent.invalid", 22, "u"))).is_none());
    }

    #[test]
    fn count_starts_at_zero_for_unused_keys() {
        // count는 전역이라 절대값을 단정할 수 없다. 지운 뒤 늘지 않는 것만 본다.
        let before = count();
        clear(PaneId::new(u64::MAX - 12));
        assert!(count() <= before);
    }
}
