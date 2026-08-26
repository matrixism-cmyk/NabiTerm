//! **되돌릴 수 없는 명령**을 알아본다 — 운영 표식이 붙은 세션에서 엔터를 막기 위한 판별기.
//!
//! 세션 표식(운영·스테이징·개발)은 만들어 두고도 **붙여넣기 하나만** 막고 있었다.
//! 손으로 친 `rm -rf /var`는 그대로 나갔다. 표식을 붙였다는 것은 "여기서는 조심해 달라"는
//! 뜻인데 그 약속을 지키지 않고 있었다.
//!
//! ## 무엇을 잡고 무엇을 놓아 주는가
//!
//! **거짓 음성을 택한다.** 뜻이 분명한 것만 잡고 애매한 것은 통과시킨다.
//! 잡지 못한 위험은 사용자가 원래 지고 있던 위험이지만, 헛확인은 우리가 새로 만든
//! 짜증이다. 확인창이 자주 헛나오면 사람은 읽지 않고 누르게 되고, 그러면 진짜일 때도
//! 그냥 누른다 — 가드가 있는 것이 없느니만 못해진다.
//!
//! 그래서 규칙은 **"이것을 실행하면 무엇이 사라지는가"가 한 줄로 설명되는 것**만 담는다.

/// 걸린 이유 — 화면에 낼 i18n 키를 고르는 데 쓴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Danger {
    /// 파일을 지운다(rm -rf, del /s …).
    Delete,
    /// 디스크·장치에 통째로 쓴다(mkfs, dd of=, > /dev/sd…).
    Disk,
    /// 기계를 내린다(shutdown, reboot, halt).
    Power,
    /// 되돌릴 수 없게 덮어쓴다(git push --force, DROP DATABASE …).
    Overwrite,
    /// 권한을 통째로 바꾼다(chmod -R 777 /, chown -R / …).
    Permission,
}

impl Danger {
    /// 화면 문구 i18n 키.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Danger::Delete => "guard.why.delete",
            Danger::Disk => "guard.why.disk",
            Danger::Power => "guard.why.power",
            Danger::Overwrite => "guard.why.overwrite",
            Danger::Permission => "guard.why.permission",
        }
    }
}

/// 이 명령줄이 되돌릴 수 없는 일을 하는가. 아니면 `None`.
///
/// 앞뒤 공백과 `sudo`·`doas`·환경변수 접두는 벗겨 내고 본다. 파이프·`&&`로 이어진
/// 줄은 **조각마다** 본다 — `cd /tmp && rm -rf *`를 놓치지 않기 위해서다.
pub(crate) fn classify(line: &str) -> Option<Danger> {
    line.split(['|', ';'])
        .flat_map(|p| p.split("&&"))
        .filter_map(|p| classify_one(p.trim()))
        .next()
}

/// 조각 하나를 본다.
fn classify_one(part: &str) -> Option<Danger> {
    let cmd = strip_prefix_noise(part);
    let low = cmd.to_ascii_lowercase();
    if low.is_empty() {
        return None;
    }
    // 지우기 — 되풀이(-r)와 강제(-f)가 **함께** 있을 때만. `rm file` 하나는 일상이다.
    if starts_with_word(&low, "rm") && has_flag(&low, 'r') && has_flag(&low, 'f') {
        return Some(Danger::Delete);
    }
    // 윈도우 쪽 대응물.
    if (starts_with_word(&low, "del") || starts_with_word(&low, "erase")) && low.contains("/s") {
        return Some(Danger::Delete);
    }
    if (starts_with_word(&low, "rd") || starts_with_word(&low, "rmdir")) && low.contains("/s") {
        return Some(Danger::Delete);
    }
    if low.contains("remove-item") && low.contains("-recurse") && low.contains("-force") {
        return Some(Danger::Delete);
    }
    // 디스크·장치.
    if starts_with_word(&low, "mkfs") || low.starts_with("mkfs.") || starts_with_word(&low, "fdisk") {
        return Some(Danger::Disk);
    }
    if starts_with_word(&low, "dd") && low.contains("of=") {
        return Some(Danger::Disk);
    }
    if low.contains("> /dev/sd") || low.contains(">/dev/sd") || starts_with_word(&low, "format") {
        return Some(Danger::Disk);
    }
    // 전원.
    for w in ["shutdown", "reboot", "halt", "poweroff"] {
        if starts_with_word(&low, w) {
            return Some(Danger::Power);
        }
    }
    // 되돌릴 수 없는 덮어쓰기.
    if low.contains("push") && (low.contains("--force") || has_flag(&low, 'f')) && starts_with_word(&low, "git") {
        return Some(Danger::Overwrite);
    }
    if starts_with_word(&low, "git") && low.contains("reset") && low.contains("--hard") {
        return Some(Danger::Overwrite);
    }
    if low.contains("drop database") || low.contains("drop table") || low.contains("truncate table") {
        return Some(Danger::Overwrite);
    }
    // 권한.
    if (starts_with_word(&low, "chmod") || starts_with_word(&low, "chown")) && has_flag(&low, 'r') && touches_root(&low) {
        return Some(Danger::Permission);
    }
    None
}

/// `sudo` · `doas` · `FOO=bar` 같은 접두를 벗긴다.
fn strip_prefix_noise(s: &str) -> &str {
    let mut cur = s.trim();
    loop {
        let head = cur.split_whitespace().next().unwrap_or("");
        let is_env = head.contains('=') && !head.starts_with('-');
        if head == "sudo" || head == "doas" || head == "time" || is_env {
            cur = cur[head.len()..].trim_start();
            continue;
        }
        return cur;
    }
}

/// 첫 낱말이 이것인가(부분 일치로 `rman`을 `rm`으로 보지 않게).
fn starts_with_word(low: &str, word: &str) -> bool {
    low.split_whitespace().next().is_some_and(|w| w == word || w.ends_with(&format!("/{word}")))
}

/// 짧은 플래그가 들어 있는가 — `-rf` 처럼 뭉친 것도 본다.
fn has_flag(low: &str, f: char) -> bool {
    low.split_whitespace()
        .filter(|w| w.starts_with('-') && !w.starts_with("--"))
        .any(|w| w.chars().skip(1).any(|c| c == f))
        || low.split_whitespace().any(|w| w == format!("--{}", if f == 'r' { "recursive" } else { "force" }))
}

/// 루트나 그 한 단계 아래를 건드리는가.
fn touches_root(low: &str) -> bool {
    low.split_whitespace().any(|w| w == "/" || (w.starts_with('/') && w.matches('/').count() == 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_classic_one_is_caught() {
        assert_eq!(classify("rm -rf /var/lib"), Some(Danger::Delete));
        assert_eq!(classify("sudo rm -rf /"), Some(Danger::Delete));
        assert_eq!(classify("rm -fr ./build"), Some(Danger::Delete));
    }

    /// **일상 명령은 통과시킨다.** 헛확인이 잦으면 사람은 읽지 않고 누르게 된다.
    #[test]
    fn everyday_commands_pass_through() {
        for ok in [
            "rm file.txt",
            "rm -r build",          // 강제(-f)가 없다
            "ls -la /",
            "git push",
            "git status",
            "cat /etc/hosts",
            "cd /var/log",
            "grep -rf pattern .",   // rm 이 아니다
            "",
            "   ",
        ] {
            assert_eq!(classify(ok), None, "{ok:?} 를 잘못 잡았다");
        }
    }

    /// 이어 붙인 줄은 **조각마다** 본다 — 앞쪽만 보면 놓친다.
    #[test]
    fn chained_commands_are_each_examined() {
        assert_eq!(classify("cd /tmp && rm -rf *"), Some(Danger::Delete));
        assert_eq!(classify("echo hi; sudo reboot"), Some(Danger::Power));
        assert_eq!(classify("cat x | grep y"), None);
    }

    #[test]
    fn disk_writes_are_caught() {
        assert_eq!(classify("mkfs.ext4 /dev/sda1"), Some(Danger::Disk));
        assert_eq!(classify("dd if=x.img of=/dev/sda"), Some(Danger::Disk));
        assert_eq!(classify("dd if=/dev/sda of=backup.img"), Some(Danger::Disk));
    }

    #[test]
    fn power_commands_are_caught() {
        assert_eq!(classify("shutdown -h now"), Some(Danger::Power));
        assert_eq!(classify("reboot"), Some(Danger::Power));
        assert_eq!(classify("poweroff"), Some(Danger::Power));
    }

    #[test]
    fn irreversible_overwrites_are_caught() {
        assert_eq!(classify("git push --force origin main"), Some(Danger::Overwrite));
        assert_eq!(classify("git reset --hard HEAD~3"), Some(Danger::Overwrite));
        assert_eq!(classify("DROP DATABASE prod;"), Some(Danger::Overwrite));
    }

    /// 권한은 **루트를 건드릴 때만** — `chmod -R 755 ./www` 는 일상이다.
    #[test]
    fn permission_changes_only_matter_at_the_root() {
        assert_eq!(classify("chmod -R 777 /"), Some(Danger::Permission));
        assert_eq!(classify("chown -R nobody /etc"), Some(Danger::Permission));
        assert_eq!(classify("chmod -R 755 ./www"), None);
    }

    /// 환경변수·`sudo` 접두가 있어도 알아본다.
    #[test]
    fn prefixes_do_not_hide_the_command() {
        assert_eq!(classify("LANG=C sudo rm -rf /opt/app"), Some(Danger::Delete));
        assert_eq!(classify("/bin/rm -rf /opt"), Some(Danger::Delete));
    }

    #[test]
    fn windows_shapes_are_caught_too() {
        assert_eq!(classify("del /s /q C:\\temp"), Some(Danger::Delete));
        assert_eq!(classify("Remove-Item -Recurse -Force C:\\build"), Some(Danger::Delete));
        assert_eq!(classify("Remove-Item build"), None);
    }

    /// 이유마다 다른 문구가 붙어야 한다(전부 같은 말이면 알려 주는 것이 없다).
    #[test]
    fn every_reason_has_its_own_wording() {
        let all = [Danger::Delete, Danger::Disk, Danger::Power, Danger::Overwrite, Danger::Permission];
        let mut keys: Vec<&str> = all.iter().map(|d| d.key()).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "이유가 문구를 나눠 쓴다");
    }
}
