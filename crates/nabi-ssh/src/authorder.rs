//! **어떤 키가 있고 어느 것부터 쓰이는가** — OpenSSH 10.5 가 `ssh -Z` 로 답하는 질문.
//!
//! ## 왜 필요한가
//!
//! SSH 가 안 붙을 때 가장 잦은 물음은 "왜 내 키로 안 되지"다. 그런데 화면에는
//! "인증 실패" 한 줄뿐이라, 사용자는 **어떤 키가 쓰였는지조차 모른 채** 짐작으로 고친다.
//!
//! OpenSSH 도 같은 문제를 겪어 10.5 에서 `ssh -Z` 를 넣었다 — 붙지는 않고 **쓰게 될 키를
//! 순서대로 보여 주기만** 한다(2026-09-01 조사). 여기가 그것에 해당한다.
//!
//! ## 우리는 아직 하나만 시도한다
//!
//! OpenSSH 는 고른 키가 안 되면 기본 이름들을 차례로 더 시도한다. 우리는 고른 것 하나로
//! 끝난다. 그래서 이 목록은 "시도한 순서"가 아니라 **"쓸 수 있었던 것들"**이다 —
//! 그 사실을 감추지 않고 화면에도 그대로 적는다. 여러 개를 시도하는 일은 인증 경로를
//! 건드리므로 실서버 검증과 함께 따로 한다.
//!
//! ## `id_dsa` 는 목록에 없다
//!
//! OpenSSH 10.0 이 DSA 를 통째로 걷어냈다(2015년부터 예고한 것을 끝낸 것이다).
//! 없는 것을 권하면 안 되므로 기본 이름에서도 뺐다.

use std::path::Path;

/// 이 후보가 어디서 왔나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// 사용자가 연결 설정에서 고른 키 — 실제로 쓰인 것.
    Chosen,
    /// `~/.ssh` 에 있는 OpenSSH 기본 이름 — 있지만 아직 안 쓴 것.
    Default,
}

/// 쓸 수 있는 키 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// 파일 이름(경로가 아니라 이름 — 화면이 좁고, 폴더는 하나뿐이다).
    pub name: String,
    pub source: Source,
    /// 짝이 되는 `.pub` 가 옆에 있는가. 없으면 서버에 등록할 것을 꺼내기가 번거롭다.
    pub has_pub: bool,
}

/// OpenSSH 가 기본으로 찾는 신원 파일 — `ssh_config(5)` 에 적힌 순서 그대로.
pub const DEFAULT_IDENTITIES: [&str; 5] =
    ["id_rsa", "id_ecdsa", "id_ecdsa_sk", "id_ed25519", "id_ed25519_sk"];

/// 고른 키를 맨 앞에 두고, `~/.ssh` 에 실제로 있는 기본 이름을 순서대로 잇는다.
///
/// `chosen` 은 전체 경로여도 되고 이름이어도 된다(이름만 떼어 견준다).
/// `present` 는 그 폴더에 있는 **모든** 파일 이름이다 — `.pub` 짝을 여기서 찾기 때문이다.
pub fn order(chosen: Option<&str>, present: &[String]) -> Vec<Candidate> {
    let picked = chosen.map(base_name);
    let has_pub = |n: &str| present.iter().any(|p| p == &format!("{n}.pub"));
    let mut out = Vec::new();
    if let Some(n) = &picked {
        out.push(Candidate { name: n.clone(), source: Source::Chosen, has_pub: has_pub(n) });
    }
    for d in DEFAULT_IDENTITIES {
        // 고른 것과 같은 파일을 두 번 적지 않는다.
        if picked.as_deref() == Some(d) || !present.iter().any(|p| p == d) {
            continue;
        }
        out.push(Candidate { name: d.to_string(), source: Source::Default, has_pub: has_pub(d) });
    }
    out
}

/// 경로에서 파일 이름만. 윈도우 역빗금과 유닉스 빗금을 모두 자른다.
fn base_name(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// 이 사용자의 `~/.ssh` 폴더. 윈도우는 `%USERPROFILE%`, 그 밖에는 `$HOME` 이다.
///
/// 찾지 못하면 `.ssh` 를 상대경로로 돌려준다 — [`scan`] 이 없는 폴더를 빈 목록으로
/// 받아 주므로, 여기서 굳이 실패를 만들 이유가 없다(실패 화면을 만들다 또 실패하면 안 된다).
pub fn default_dir() -> std::path::PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    home.map(std::path::PathBuf::from).unwrap_or_default().join(".ssh")
}

/// `~/.ssh` 같은 폴더의 파일 이름들. 폴더가 없거나 못 읽으면 빈 목록이다.
///
/// **상한을 둔다.** 이 목록은 실패 화면 한 줄을 만들려고 읽는 것인데, 누군가 그 폴더에
/// 파일 수만 개를 두었다면 화면 한 줄 때문에 오래 멈출 이유가 없다.
pub fn scan(dir: &Path) -> Vec<String> {
    const CAP: usize = 512;
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        if out.len() >= CAP {
            break;
        }
        out.push(e.file_name().to_string_lossy().into_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// 고른 키가 언제나 맨 앞이다 — 실제로 쓰인 것이 먼저 보여야 한다.
    #[test]
    fn the_chosen_key_comes_first() {
        let present = names(&["id_rsa", "id_ed25519", "mykey", "mykey.pub"]);
        let got = order(Some(r"C:\Users\x\.ssh\mykey"), &present);
        assert_eq!(got[0].name, "mykey");
        assert_eq!(got[0].source, Source::Chosen);
        assert!(got[0].has_pub, ".pub 가 옆에 있는데 없다고 했다");
    }

    /// 기본 이름은 ssh_config 에 적힌 순서를 지킨다 — 알파벳 순이 아니다.
    #[test]
    fn defaults_keep_the_documented_order() {
        let present = names(&["id_ed25519", "id_rsa", "id_ecdsa"]);
        let got: Vec<String> = order(None, &present).into_iter().map(|c| c.name).collect();
        assert_eq!(got, ["id_rsa", "id_ecdsa", "id_ed25519"]);
    }

    /// 없는 파일은 권하지 않는다. 폴더에 있는 것만 나온다.
    #[test]
    fn only_files_that_are_really_there_are_listed() {
        assert!(order(None, &names(&["known_hosts", "config"])).is_empty());
    }

    /// 고른 키가 기본 이름과 같으면 한 번만 나온다.
    #[test]
    fn the_same_file_is_not_listed_twice() {
        let present = names(&["id_ed25519", "id_ed25519.pub"]);
        let got = order(Some("id_ed25519"), &present);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].source, Source::Chosen, "고른 쪽으로 남아야 한다");
    }

    /// **DSA 는 권하지 않는다** — OpenSSH 10.0 이 걷어냈다.
    #[test]
    fn dsa_is_never_suggested() {
        assert!(!DEFAULT_IDENTITIES.contains(&"id_dsa"));
        assert!(order(None, &names(&["id_dsa", "id_dsa.pub"])).is_empty());
    }

    /// `.pub` 가 없으면 없다고 말한다 — 서버에 등록할 것을 꺼내기가 번거로워진다.
    #[test]
    fn a_missing_public_half_is_reported() {
        let got = order(None, &names(&["id_rsa"]));
        assert_eq!(got.len(), 1);
        assert!(!got[0].has_pub);
    }

    /// 유닉스 경로에서도 이름만 떼어 낸다.
    #[test]
    fn a_unix_path_is_reduced_to_its_name() {
        assert_eq!(base_name("/home/u/.ssh/id_ed25519"), "id_ed25519");
        assert_eq!(base_name("mykey"), "mykey");
    }

    /// 폴더가 없으면 오류가 아니라 빈 목록이다 — 실패 화면을 만들다가 또 실패하면 안 된다.
    #[test]
    fn a_missing_folder_is_not_an_error() {
        assert!(scan(Path::new(r"C:\nabi-no-such-folder-9f2b")).is_empty());
    }
}
