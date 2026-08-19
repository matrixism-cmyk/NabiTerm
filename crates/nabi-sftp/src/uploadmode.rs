//! 업로드 파일 권한 정규화 — 올린 파일에 어떤 Unix 모드를 줄지 정한다.
//!
//! **"권한 보존"은 Windows에서 성립하지 않는다.** NTFS 파일에는 Unix 모드가 없어서
//! 보존할 원본 값 자체가 없다. 실제로 사람이 겪는 문제는 따로 있다 — 올린 셸 스크립트에
//! 실행 권한이 없어 `./deploy.sh`가 바로 안 된다. 그래서 보존이 아니라 **정규화**로 푼다:
//! 일반 파일은 안전한 기본값, 스크립트로 보이는 확장자는 실행 비트까지.
//!
//! 서버가 SETSTAT을 거부하면 조용히 넘어간다(전송 자체는 이미 성공했다).

/// 실행 권한을 줄 만한 스크립트 확장자(소문자 비교).
const SCRIPTS: &[&str] = &["sh", "bash", "zsh", "ksh", "py", "pl", "rb", "run"];

/// 설정 문자열과 파일 이름으로 적용할 모드를 정한다.
///
/// - 빈 문자열/`off` → `None`(아무것도 하지 않음, 기본값)
/// - `auto` → 스크립트면 0o755, 아니면 0o644
/// - 8진수 문자열(`644`, `0755`) → 그 값 그대로
pub fn mode_for(setting: &str, name: &str) -> Option<u32> {
    let s = setting.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("off") {
        return None;
    }
    if s.eq_ignore_ascii_case("auto") {
        return Some(if is_script(name) { 0o755 } else { 0o644 });
    }
    u32::from_str_radix(s.trim_start_matches("0o"), 8).ok().filter(|m| *m <= 0o7777)
}

/// 파일 이름이 실행 가능한 스크립트로 보이는가(확장자 기준, 대소문자 무시).
fn is_script(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .is_some_and(|ext| SCRIPTS.contains(&ext.as_str()))
}

#[cfg(test)]
mod tests {
    use super::mode_for;

    #[test]
    fn off_by_default() {
        assert_eq!(mode_for("", "a.sh"), None);
        assert_eq!(mode_for("off", "a.sh"), None);
        assert_eq!(mode_for("  ", "a.sh"), None);
    }

    #[test]
    fn auto_gives_exec_bit_to_scripts_only() {
        assert_eq!(mode_for("auto", "deploy.sh"), Some(0o755));
        assert_eq!(mode_for("auto", "run.PY"), Some(0o755)); // 대소문자 무시.
        assert_eq!(mode_for("auto", "notes.txt"), Some(0o644));
        assert_eq!(mode_for("auto", "Makefile"), Some(0o644)); // 확장자 없음.
    }

    #[test]
    fn explicit_octal_is_used_verbatim() {
        assert_eq!(mode_for("644", "a.sh"), Some(0o644)); // 명시값이 auto를 이긴다.
        assert_eq!(mode_for("0755", "a.txt"), Some(0o755));
    }

    #[test]
    fn nonsense_setting_is_ignored() {
        assert_eq!(mode_for("999", "a"), None); // 8진수가 아니다.
        assert_eq!(mode_for("rwxr-xr-x", "a"), None);
        assert_eq!(mode_for("77777", "a"), None); // 범위 초과.
    }
}
