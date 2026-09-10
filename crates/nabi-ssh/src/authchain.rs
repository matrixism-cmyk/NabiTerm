//! **여러 방법을 차례로 시도한다** — 지금까지는 고른 것 하나로 끝났다.
//!
//! ## 왜 필요한가
//!
//! 사용자가 세션에 키를 하나 골라 두면 우리는 그것만 써 보고, 안 되면 끝이다.
//! 그런데 실제로는 이런 일이 흔하다.
//!
//! * `~/.ssh` 에 키가 여럿인데 세션에 적어 둔 것은 옛것이다.
//! * 에이전트(Pageant·OpenSSH)에 키가 올라가 있는데 세션은 파일을 가리킨다.
//! * 서버가 그 키 종류를 아예 안 받는다.
//!
//! OpenSSH 는 이럴 때 다음 것을 시도한다. 우리는 안 했으므로, 같은 계정·같은 서버인데
//! **OpenSSH 로는 붙고 우리로는 안 붙는** 상태가 됐다. `authorder` 가 이 결함을 스스로
//! 적어 두고 있었다("우리는 아직 하나만 시도한다").
//!
//! ## 왜 무작정 다 시도하면 안 되나
//!
//! 서버는 시도 횟수를 센다. OpenSSH 기본값 `MaxAuthTries` 는 **6**이고, 넘으면 그냥
//! 끊는다. 더 나쁜 것은 계정 잠금 정책이 있는 곳이다 — 우리가 친절하답시고 키를 열 개
//! 던지면 **사용자 계정을 잠글 수 있다.** 편의 기능이 사고를 내는 건 최악이다.
//!
//! 그래서 두 가지를 지킨다.
//!
//! 1. **서버에게 먼저 묻는다.** 첫 실패(또는 `none` 시도)의 답에 서버가 받는 방법이
//!    적혀 온다. 서버가 `publickey` 를 안 받으면 키는 한 번도 안 던진다.
//! 2. **상한을 둔다**([`MAX_ATTEMPTS`]). 기본 6보다 하나 적게 잡아 여유를 남긴다.
//!
//! ## 사용자가 고른 것이 언제나 먼저다
//!
//! 우리가 순서를 "똑똑하게" 바꾸면, 사용자가 고른 키가 아닌 것으로 붙어 놓고
//! 그 사실을 모르게 된다. 고른 것을 맨 앞에 두고, 그다음에만 거든다.

use crate::authorder::{self, Source};

/// 한 번의 접속에서 시도할 수 있는 최대 횟수.
///
/// OpenSSH 서버 기본 `MaxAuthTries` 가 6이다. 하나를 남겨 둔다 — 서버가 그 값을
/// 낮춰 두었을 수도 있고, 우리가 쓰지 않은 시도가 남아 있어야 사용자가 직접
/// 한 번 더 해 볼 수 있다.
pub const MAX_ATTEMPTS: usize = 5;

/// 한 번의 시도.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    /// 에이전트(Pageant·OpenSSH)에게 서명을 부탁한다. 개인키는 넘어오지 않는다.
    Agent,
    /// 이 파일의 개인키로.
    Key { name: String, source: Source },
    /// 비밀번호. **사용자가 적어 준 것만** 쓴다 — 우리가 만들어 내지 않는다.
    Password,
}

/// 서버가 받는다고 말한 방법들.
///
/// 서버가 아무 말도 안 했으면(`None`) 걸러내지 않는다 — 모른다고 해서 아무것도
/// 안 하는 것보다, 아는 만큼 해 보고 결과를 보여 주는 편이 낫다.
#[derive(Debug, Clone, Copy, Default)]
pub struct ServerAccepts {
    pub publickey: bool,
    pub password: bool,
    /// 서버가 목록을 준 적이 있나. 없으면 위 두 값은 뜻이 없다.
    pub known: bool,
}

impl ServerAccepts {
    /// 아직 서버에게 못 물어본 상태.
    pub fn unknown() -> Self {
        Self::default()
    }

    fn allows_key(&self) -> bool {
        !self.known || self.publickey
    }

    fn allows_password(&self) -> bool {
        !self.known || self.password
    }
}

/// 무엇을 어떤 차례로 시도할지 정한다.
///
/// * `chosen_key` — 세션에 적어 둔 키(경로여도 이름이어도 된다).
/// * `has_password` — 사용자가 비밀번호를 적어 두었나.
/// * `chose_agent` — 세션이 에이전트를 고른 상태인가.
/// * `present` — `~/.ssh` 에 실제로 있는 파일 이름들.
/// * `agent_running` — 에이전트에 닿을 수 있나.
pub fn plan(
    chosen_key: Option<&str>,
    has_password: bool,
    chose_agent: bool,
    present: &[String],
    agent_running: bool,
    accepts: ServerAccepts,
) -> Vec<Attempt> {
    let mut out: Vec<Attempt> = Vec::new();
    // 1) 사용자가 고른 것. 서버가 안 받는다고 해도 **한 번은 해 본다** — 고른 것이
    //    조용히 건너뛰어지면 "왜 내 키를 안 쓰지"라는 더 나쁜 물음이 생긴다.
    if chose_agent && agent_running {
        out.push(Attempt::Agent);
    }
    if has_password {
        out.push(Attempt::Password);
    }
    for c in authorder::order(chosen_key, present) {
        if c.source != Source::Chosen {
            continue;
        }
        out.push(Attempt::Key { name: c.name, source: c.source });
    }
    // 2) 거드는 것들 — 서버가 받는다고 한 것만.
    if !chose_agent && agent_running && accepts.allows_key() {
        out.push(Attempt::Agent);
    }
    if accepts.allows_key() {
        for c in authorder::order(chosen_key, present) {
            if c.source == Source::Chosen {
                continue;
            }
            out.push(Attempt::Key { name: c.name, source: c.source });
        }
    }
    if !accepts.allows_password() {
        out.retain(|a| !matches!(a, Attempt::Password));
    }
    out.truncate(MAX_ATTEMPTS);
    out
}

/// 이 시도를 사람에게 뭐라고 말할까(진단 화면 한 줄).
pub fn label(a: &Attempt) -> String {
    match a {
        Attempt::Agent => "agent".to_string(),
        Attempt::Password => "password".to_string(),
        Attempt::Key { name, .. } => name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn all() -> ServerAccepts {
        ServerAccepts { publickey: true, password: true, known: true }
    }

    /// **고른 것이 언제나 맨 앞이다.** 순서를 우리가 똑똑하게 바꾸면, 사용자는 자기가
    /// 고르지 않은 키로 붙어 놓고 그 사실을 모르게 된다.
    #[test]
    fn 고른_것이_맨_앞이다() {
        let p = plan(Some("work_key"), false, false, &names(&["work_key", "id_rsa", "id_ed25519"]), true, all());
        assert_eq!(p.first(), Some(&Attempt::Key { name: "work_key".into(), source: Source::Chosen }));
    }

    /// 서버가 `publickey` 를 안 받으면 키를 던지지 않는다 — 던져 봐야 횟수만 깎인다.
    #[test]
    fn 서버가_안_받는_방법은_안_쓴다() {
        let no_key = ServerAccepts { publickey: false, password: true, known: true };
        let p = plan(None, true, false, &names(&["id_rsa", "id_ed25519"]), true, no_key);
        assert_eq!(p, vec![Attempt::Password], "{p:?}");
    }

    /// 비밀번호를 서버가 안 받으면 목록에서 빠진다(사용자가 적어 두었더라도).
    #[test]
    fn 서버가_비밀번호를_안_받으면_뺀다() {
        let no_pw = ServerAccepts { publickey: true, password: false, known: true };
        let p = plan(None, true, false, &names(&["id_rsa"]), false, no_pw);
        assert!(!p.contains(&Attempt::Password), "{p:?}");
    }

    /// **상한을 넘지 않는다.** 넘으면 서버가 끊고, 계정 잠금 정책이 있는 곳에서는
    /// 우리가 사용자 계정을 잠근다.
    #[test]
    fn 상한을_넘지_않는다() {
        let many = names(&["id_rsa", "id_ecdsa", "id_ecdsa_sk", "id_ed25519", "id_ed25519_sk", "extra"]);
        let p = plan(Some("extra"), true, true, &many, true, all());
        assert!(p.len() <= MAX_ATTEMPTS, "{} 번 시도하려 한다: {p:?}", p.len());
    }

    /// 서버에게 아직 못 물어봤으면 걸러내지 않는다 — 모른다고 아무것도 안 하면 안 된다.
    #[test]
    fn 모를_때는_걸러내지_않는다() {
        let p = plan(None, true, false, &names(&["id_rsa"]), true, ServerAccepts::unknown());
        assert!(p.contains(&Attempt::Password), "{p:?}");
        assert!(p.iter().any(|a| matches!(a, Attempt::Key { .. })), "{p:?}");
    }

    /// 에이전트가 없으면 목록에도 없다 — 없는 것을 시도하면 횟수만 깎인다.
    #[test]
    fn 에이전트가_없으면_안_넣는다() {
        let p = plan(None, false, true, &names(&["id_rsa"]), false, all());
        assert!(!p.contains(&Attempt::Agent), "{p:?}");
    }

    /// 같은 키를 두 번 던지지 않는다(고른 것이 기본 이름 중 하나일 때).
    #[test]
    fn 같은_키를_두_번_안_던진다() {
        let p = plan(Some("id_rsa"), false, false, &names(&["id_rsa", "id_ed25519"]), false, all());
        let rsa = p.iter().filter(|a| matches!(a, Attempt::Key { name, .. } if name == "id_rsa")).count();
        assert_eq!(rsa, 1, "{p:?}");
    }

    /// 아무것도 없으면 빈 계획이다 — 빈 계획은 "붙일 방법이 없다"는 뜻이고,
    /// 그 말을 사용자에게 해 줄 수 있다(조용히 실패하는 것보다 낫다).
    #[test]
    fn 아무것도_없으면_빈_계획() {
        assert!(plan(None, false, false, &[], false, all()).is_empty());
    }
}
