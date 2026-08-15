//! `nabi cli integration install|status claude`(A5) — 세션 ID 보고 훅 자동 설치.
//!
//! Claude Code의 SessionStart 훅에 스크립트를 등록해, 세션이 시작될 때마다
//! `nabi cli agent session <id>`로 자기 세션 ID를 보고하게 한다. 그러면 워크스페이스
//! 복원이 `claude --resume <id>`로 **그 세션을 정확히** 이어간다(A6과 한 쌍).
//!
//! 사용자 settings.json을 수정하므로: ① 먼저 .bak 백업 ② JSON 병합(다른 설정 보존)
//! ③ 이미 등록돼 있으면 아무것도 바꾸지 않는다(멱등).

use serde_json::{json, Value};
use std::path::PathBuf;

fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".claude"))
}

/// 훅 스크립트 본문 — stdin JSON에서 session_id를 꺼내 보고한다. 실패는 조용히(훅이
/// 에이전트 실행을 방해하면 안 된다).
fn hook_script(exe: &str) -> String {
    format!(
        "$in = [Console]::In.ReadToEnd()\r\n\
         try {{\r\n  $j = $in | ConvertFrom-Json\r\n  if ($j.session_id) {{ & \"{exe}\" cli agent session \"$($j.session_id)\" | Out-Null }}\r\n}} catch {{}}\r\n"
    )
}

fn hook_command(script: &std::path::Path) -> String {
    format!("powershell -NoLogo -NoProfile -File \"{}\"", script.display())
}

/// 설치. 반환=사람이 읽을 결과 메시지.
pub fn install_claude() -> Result<String, String> {
    let dir = claude_dir().ok_or("USERPROFILE 없음")?;
    let hooks_dir = dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string();
    let script = hooks_dir.join("nabiterm-session.ps1");
    std::fs::write(&script, hook_script(&exe)).map_err(|e| e.to_string())?;

    let settings = dir.join("settings.json");
    let mut root: Value = std::fs::read_to_string(&settings)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let cmd = hook_command(&script);
    if already_installed(&root, &cmd) {
        return Ok("이미 설치됨(변경 없음)".into());
    }
    // 백업은 실제로 바꿀 때만(불필요한 .bak 양산 방지).
    if settings.exists() {
        let _ = std::fs::copy(&settings, settings.with_extension("json.bak"));
    }
    let entry = json!({ "hooks": [{ "type": "command", "command": cmd }] });
    let arr = root
        .as_object_mut()
        .ok_or("settings.json 최상위가 객체가 아님")?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("hooks가 객체가 아님")?
        .entry("SessionStart")
        .or_insert_with(|| json!([]));
    arr.as_array_mut().ok_or("SessionStart가 배열이 아님")?.push(entry);
    std::fs::write(&settings, serde_json::to_string_pretty(&root).unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(format!("설치됨: {} + settings.json SessionStart 훅(백업 .bak)", script.display()))
}

/// 설치 여부 점검(파일 존재 + settings 등록).
pub fn status_claude() -> String {
    let Some(dir) = claude_dir() else { return "USERPROFILE 없음".into() };
    let script = dir.join("hooks").join("nabiterm-session.ps1");
    let root: Value = std::fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let registered = script.exists() && already_installed(&root, &hook_command(&script));
    if registered {
        "claude: 설치됨(SessionStart → agent session 보고)".into()
    } else {
        "claude: 미설치 — nabi cli integration install claude".into()
    }
}

fn already_installed(root: &Value, cmd: &str) -> bool {
    root["hooks"]["SessionStart"]
        .as_array()
        .is_some_and(|a| a.iter().any(|e| {
            e["hooks"].as_array().is_some_and(|hs| hs.iter().any(|h| h["command"] == cmd))
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 멱등 판정: 같은 명령이 이미 있으면 설치돼 있다고 본다(중복 등록 방지의 근거).
    #[test]
    fn detects_existing_registration() {
        let cmd = "powershell -NoLogo -NoProfile -File \"C:\\x\\nabiterm-session.ps1\"";
        let root = json!({ "hooks": { "SessionStart": [
            { "hooks": [{ "type": "command", "command": cmd }] },
            { "hooks": [{ "type": "command", "command": "other" }] }
        ]}});
        assert!(already_installed(&root, cmd));
        assert!(!already_installed(&root, "powershell missing"));
        assert!(!already_installed(&json!({}), cmd));
    }

    /// 훅 스크립트는 실패를 조용히 삼킨다(에이전트 실행을 막지 않는 것이 우선).
    #[test]
    fn hook_script_swallows_errors() {
        let s = hook_script(r"C:\p\nabi.exe");
        assert!(s.contains("catch {}"));
        assert!(s.contains("agent session"));
    }
}
