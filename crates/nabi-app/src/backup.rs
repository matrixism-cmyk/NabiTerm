//! 설정 **전체 백업·복원** — PC를 옮겨도 다시 꾸미지 않게.
//!
//! 지금까지는 세션만 내보낼 수 있었다(`nabi-session/src/export.rs`). 설정·nabiPad 설정·
//! 스니펫·known_hosts는 빠져 있어, 새 PC에서는 글꼴부터 단축키까지 처음부터 다시 맞춰야
//! 했다(감사 2026-08-25).
//!
//! ## 무엇을 담고 무엇을 뺐는가
//!
//! 담는 것: `config.toml`(스니펫 포함) · `nabipad.toml` · `sessions.toml` · `known_hosts`.
//!
//! **볼트(`vault.bin`)는 담지 않는다.** 그 안에 SSH 비밀번호가 들어 있고, 백업 파일은
//! 메일이나 USB로 흘러 다니게 마련이다. 마스터 비밀번호로 암호화돼 있다 해도, 파일이
//! 새는 순간 남는 방어선은 그 비밀번호 하나뿐이다. 자격증명은 새 PC에서 다시 넣는 편이
//! 낫고, 그렇게 정해 두면 사용자도 백업 파일을 다루는 마음가짐이 달라진다.
//!
//! 형식은 사람이 읽을 수 있는 JSON 한 덩어리다. zip이 아니라 한 파일이라 메일에 붙이기
//! 쉽고, 무엇이 들었는지 열어서 확인할 수 있다.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 백업 파일 한 덩어리. 없는 항목은 비워 둔다(부분 복원 가능).
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Backup {
    /// 만든 프로그램 판(복원할 때 참고용 — 막지는 않는다).
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_hosts: Option<String>,
}

/// 어떤 항목이 어느 파일로 가는지 — 만들기와 되돌리기가 이 표 하나를 공유한다(SSOT).
type Slot = (
    fn(&Backup) -> Option<&String>,
    fn(&mut Backup, String),
    fn(&nabi_config::StorageLayout) -> std::path::PathBuf,
);

fn slots() -> [Slot; 4] {
    [
        (|b| b.config.as_ref(), |b, v| b.config = Some(v), |l| l.config_file.clone()),
        (|b| b.editor.as_ref(), |b, v| b.editor = Some(v), |l| l.editor_file.clone()),
        (|b| b.sessions.as_ref(), |b, v| b.sessions = Some(v), |l| l.sessions_file.clone()),
        (|b| b.known_hosts.as_ref(), |b, v| b.known_hosts = Some(v), |l| l.known_hosts.clone()),
    ]
}

/// 지금 설정을 모아 백업 한 덩어리를 만든다.
///
/// **못 읽은 파일의 이름을 함께 돌려준다**(배치 AF). 예전에는 조용히 건너뛰었다 — 그러면
/// 빠진 채로 "백업 완료"가 뜨고, 사용자는 되돌릴 때가 되어서야 없다는 것을 안다.
/// 그때는 원본도 이미 사라졌을 수 있다.
///
/// **"없는 파일"은 알리지 않는다.** 세션을 한 번도 만들지 않았으면 `sessions.toml` 은
/// 원래 없다. 그것까지 경고하면 알림이 소음이 되고, 소음이 되면 진짜 경고도 안 읽는다.
pub(crate) fn collect(layout: &nabi_config::StorageLayout) -> (Backup, Vec<String>) {
    let mut b = Backup { version: env!("CARGO_PKG_VERSION").to_string(), ..Default::default() };
    let mut failed = Vec::new();
    for (_, set, path) in slots() {
        let p = path(layout);
        match std::fs::read_to_string(&p) {
            Ok(text) => set(&mut b, text),
            // 아직 만들지 않은 설정은 빠진 것이 아니다.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => failed.push(format!("{}: {e}", p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default())),
        }
    }
    (b, failed)
}

/// 백업을 설정 폴더에 되돌린다. 되돌린 항목 수를 셈해 돌려준다.
///
/// **덮어쓰기 전에 기존 파일을 `.bak`으로 옮겨 둔다.** 복원은 되돌릴 수 없는 동작이라,
/// 엉뚱한 파일을 골랐을 때 원래대로 갈 길이 있어야 한다.
pub(crate) fn restore(b: &Backup, layout: &nabi_config::StorageLayout) -> std::io::Result<usize> {
    std::fs::create_dir_all(&layout.base)?;
    let mut n = 0;
    for (get, _, path) in slots() {
        let Some(text) = get(b) else { continue };
        let p = path(layout);
        if p.exists() {
            // **밀어 두기가 실패하면 덮어쓰지 않는다**(배치 AG). 위 주석이 약속한 "원래대로
            // 갈 길"이 바로 이 `.bak` 이고, 그것을 못 만든 채 덮어쓰면 그 길이 사라진다.
            //
            // 예전에는 `let _ =` 로 삼키고 그대로 덮어썼다. 되돌릴 수 없는 동작에서
            // 안전망을 잃은 것을 말하지 않는 것이 가장 나쁘다 — 사용자는 안전망이 있다고
            // 믿고 복원을 누른다.
            std::fs::rename(&p, backup_name(&p))?;
        }
        std::fs::write(&p, text)?;
        n += 1;
    }
    Ok(n)
}

/// 덮어쓰기 전에 밀어 둘 이름(`config.toml` → `config.toml.bak`).
fn backup_name(p: &Path) -> std::path::PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(".bak");
    std::path::PathBuf::from(s)
}

/// 백업을 파일 텍스트로.
pub(crate) fn to_text(b: &Backup) -> String {
    serde_json::to_string_pretty(b).unwrap_or_default()
}

/// 파일 텍스트를 백업으로. 형식이 아니면 None.
pub(crate) fn from_text(t: &str) -> Option<Backup> {
    serde_json::from_str(t).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(tag: &str) -> nabi_config::StorageLayout {
        let base = std::env::temp_dir().join(format!("nabi-backup-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        nabi_config::StorageLayout::from_base(base)
    }

    fn seed(l: &nabi_config::StorageLayout) {
        std::fs::write(&l.config_file, "[appearance]\nfont_size = 15.0\n").unwrap();
        std::fs::write(&l.editor_file, "tab_size = 2\n").unwrap();
        std::fs::write(&l.sessions_file, "# sessions\n").unwrap();
        std::fs::write(&l.known_hosts, "example.com ssh-ed25519 AAAA\n").unwrap();
    }

    #[test]
    fn a_backup_round_trips_through_a_file() {
        let a = layout("rt-a");
        seed(&a);
        let text = to_text(&collect(&a).0);
        let b = layout("rt-b");
        let got = from_text(&text).expect("다시 읽혀야 한다");
        assert_eq!(restore(&got, &b).unwrap(), 4);
        assert_eq!(std::fs::read_to_string(&b.config_file).unwrap(), "[appearance]\nfont_size = 15.0\n");
        assert_eq!(std::fs::read_to_string(&b.known_hosts).unwrap(), "example.com ssh-ed25519 AAAA\n");
        for l in [a, b] {
            let _ = std::fs::remove_dir_all(&l.base);
        }
    }

    /// **볼트는 절대 담기지 않는다.** 백업 파일은 흘러 다니고, 그 안에 비밀번호가 있으면 안 된다.
    #[test]
    fn the_vault_is_never_included() {
        let l = layout("vault");
        seed(&l);
        std::fs::write(&l.vault, "SECRET-VAULT-BYTES").unwrap();
        let text = to_text(&collect(&l).0);
        assert!(!text.contains("SECRET-VAULT-BYTES"), "볼트 내용이 백업에 섞였다");
        assert!(!text.contains("vault"), "볼트 항목 자체가 없어야 한다");
        let _ = std::fs::remove_dir_all(&l.base);
    }

    /// 복원은 기존 파일을 `.bak`으로 밀어 둔다 — 잘못 골랐을 때 돌아갈 길.
    #[test]
    fn restoring_keeps_the_previous_files() {
        let l = layout("keep");
        seed(&l);
        let old = std::fs::read_to_string(&l.config_file).unwrap();
        let b = Backup { config: Some("[appearance]\nfont_size = 99.0\n".into()), ..Default::default() };
        assert_eq!(restore(&b, &l).unwrap(), 1);
        assert!(std::fs::read_to_string(&l.config_file).unwrap().contains("99.0"));
        let kept = std::fs::read_to_string(backup_name(&l.config_file)).unwrap();
        assert_eq!(kept, old, "덮어쓰기 전 파일이 .bak으로 남아야 한다");
        let _ = std::fs::remove_dir_all(&l.base);
    }

    /// 일부만 든 백업도 그만큼만 되돌린다(옛 판·부분 백업 호환).
    #[test]
    fn a_partial_backup_restores_only_what_it_has() {
        let l = layout("partial");
        let got = from_text(r##"{"version":"0.0.1","sessions":"# only"}"##).unwrap();
        assert_eq!(restore(&got, &l).unwrap(), 1);
        assert!(l.sessions_file.exists());
        assert!(!l.config_file.exists());
        let _ = std::fs::remove_dir_all(&l.base);
    }

    #[test]
    fn junk_is_not_mistaken_for_a_backup() {
        assert!(from_text("이건 백업이 아니다").is_none());
        assert!(from_text("").is_none());
    }
    #[test]
    fn a_config_that_was_never_created_is_not_reported_as_missing() {
        // 세션을 한 번도 만들지 않았으면 sessions.toml 은 원래 없다. 그것까지 경고하면
        // 알림이 소음이 되고, 소음이 되면 진짜 경고도 안 읽는다.
        let l = layout("none"); // 아무것도 만들지 않은 빈 폴더.
        let (_, failed) = collect(&l);
        assert!(failed.is_empty(), "없는 파일은 실패가 아니다: {failed:?}");
    }

    #[test]
    fn a_directory_where_a_file_should_be_is_reported() {
        // 읽기가 실패하는 경우를 만든다 — 파일 자리에 폴더가 있으면 read_to_string 이 실패한다.
        // 조용히 건너뛰면 빠진 채로 "백업 완료"가 뜨고, 되돌릴 때가 되어서야 알게 된다.
        let l = layout("unreadable");
        std::fs::create_dir_all(&l.config_file).unwrap(); // 파일 자리에 폴더.
        let (_, failed) = collect(&l);
        assert_eq!(failed.len(), 1, "못 읽은 것을 알려야 한다: {failed:?}");
        assert!(failed[0].contains("config"), "어느 파일인지 말해야 한다: {failed:?}");
    }

    #[test]
    fn a_restore_stops_when_the_original_cannot_be_set_aside() {
        // 주석이 약속한 "원래대로 갈 길"이 .bak 이다. 그것을 못 만든 채 덮어쓰면 그 길이
        // 사라지는데, 예전에는 실패를 삼키고 그대로 덮어썼다.
        let l = layout("nobak");
        seed(&l);
        // .bak 자리에 폴더를 두면 rename 이 실패한다.
        std::fs::create_dir_all(super::backup_name(&l.config_file)).unwrap();
        let (b, _) = collect(&l);
        let before = std::fs::read_to_string(&l.config_file).unwrap();
        assert!(restore(&b, &l).is_err(), "밀어 두기가 실패하면 복원도 실패해야 한다");
        assert_eq!(std::fs::read_to_string(&l.config_file).unwrap(), before, "원본이 그대로 남아야 한다");
    }

}
