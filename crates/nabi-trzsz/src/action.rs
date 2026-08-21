//! `#ACT`(우리가 보냄)와 `#CFG`(원격이 보냄)의 JSON 본문.
//!
//! 키 이름은 원격 구현이 그대로 읽는 **약속**이라 바꿀 수 없다(`binary`가 곧
//! `support_binary`인 것처럼 이름과 뜻이 어긋나는 자리도 원본을 그대로 따른다).

use serde::{Deserialize, Serialize};

/// 우리가 전송을 받아들일지 알린다. `confirm:false`면 원격 `trz`/`tsz`가 스스로 끝난다.
#[derive(Debug, Clone, Serialize)]
pub struct Action {
    pub lang: &'static str,
    pub version: &'static str,
    pub confirm: bool,
    /// 원격이 Windows면 `"!\n"`. 그쪽 cmd가 단독 `\n`을 흘리지 못한다.
    pub newline: String,
    pub protocol: u32,
    /// 이스케이프 바이너리 모드 지원 여부(키 이름이 `binary`인 것은 원본 그대로다).
    #[serde(rename = "binary")]
    pub support_binary: bool,
    #[serde(rename = "support_dir")]
    pub support_directory: bool,
}

impl Action {
    /// 받아들임/거절을 만든다. 지금은 프로토콜 v1 + base64 모드만 쓴다(§계획 P6에서 확장).
    pub fn new(confirm: bool, win_server: bool) -> Self {
        Self {
            lang: crate::CLIENT_LANG,
            version: crate::CLIENT_VERSION,
            confirm,
            newline: if win_server { "!\n".into() } else { "\n".into() },
            protocol: crate::PROTOCOL_VERSION,
            support_binary: false,
            support_directory: true,
        }
    }
}

/// 원격이 정해서 알려주는 전송 설정. **원격이 준 값이므로 그대로 믿지 않는다** —
/// 상한이 필요한 값은 여기서 조인다.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub quiet: bool,
    pub binary: bool,
    pub directory: bool,
    pub overwrite: bool,
    pub timeout: u32,
    /// 원격이 **명시했을 때만** 값이 있다. 없으면 `#ACT`에서 정한 값을 그대로 쓴다.
    ///
    /// 여기서 기본값으로 덮어쓰면 Windows 원격과 어긋나 전송이 그 자리에서 멈춘다 —
    /// 실서버 검증(2026-08-21, 파이썬 trzsz 1.1.5)에서 정확히 이 결함이 나왔다.
    /// 원격은 `!\n`으로 말하는데 우리는 CFG를 읽는 순간 `\n`으로 되돌려 답하고 있었다.
    pub newline: Option<String>,
    pub protocol: u32,
    #[serde(rename = "bufsize")]
    pub max_buf_size: i64,
    pub tmux_output_junk: bool,
    /// 모드 `F`에서 원격이 **올리라고 지정한 로컬 파일** — 기본 차단 대상이다.
    pub client_files: Vec<String>,
}

/// 한 청크의 기본 크기. 원격이 더 큰 값을 알려줘도 이 위로는 올리지 않는다.
pub const CHUNK_START: usize = 1024;
/// 청크 상한. base64+zlib을 거치므로 실제 줄은 이보다 커진다.
pub const CHUNK_MAX: usize = 1 << 20;
/// 응답을 기다리는 기본 시간(초). 원격이 더 짧게 주면 그 값을 쓴다.
pub const TIMEOUT_DEFAULT: u32 = 100;

impl Config {
    /// 원격이 준 설정을 우리 상한으로 조인 값.
    pub fn sanitized(mut self) -> Self {
        // 아는 줄바꿈만 받아들인다. 모르는 값이면 없는 것으로 쳐서 협상값을 지킨다.
        if !matches!(self.newline.as_deref(), Some("\n") | Some("!\n")) {
            self.newline = None;
        }
        if self.protocol == 0 {
            self.protocol = 1;
        }
        self.max_buf_size = self.max_buf_size.clamp(CHUNK_START as i64, CHUNK_MAX as i64);
        if self.timeout == 0 || self.timeout > 3600 {
            self.timeout = TIMEOUT_DEFAULT;
        }
        self
    }

    /// 이번 전송에서 쓸 청크 상한.
    pub fn chunk_max(&self) -> usize {
        self.max_buf_size.clamp(CHUNK_START as i64, CHUNK_MAX as i64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_uses_the_protocol_key_names() {
        let s = serde_json::to_string(&Action::new(true, false)).unwrap();
        assert!(s.contains("\"binary\":false"), "support_binary는 binary로 나가야 한다: {s}");
        assert!(s.contains("\"support_dir\":true"), "{s}");
        assert!(s.contains("\"protocol\":1"), "{s}");
        assert!(s.contains(r#""newline":"\n""#), "{s}");
    }

    #[test]
    fn windows_remote_gets_the_bang_newline() {
        let s = serde_json::to_string(&Action::new(true, true)).unwrap();
        assert!(s.contains(r#""newline":"!\n""#), "{s}");
    }

    #[test]
    fn config_defaults_are_filled_in() {
        let c: Config = serde_json::from_str("{}").unwrap();
        let c = c.sanitized();
        assert_eq!(c.newline, None, "원격이 말하지 않았으면 협상값을 지킨다");
        assert_eq!(c.protocol, 1);
        assert_eq!(c.timeout, TIMEOUT_DEFAULT);
        assert_eq!(c.chunk_max(), CHUNK_START);
    }

    /// 원격이 준 값은 그대로 믿지 않는다 — 메모리를 원격이 정하게 두면 안 된다.
    #[test]
    fn absurd_remote_values_are_clamped() {
        let c: Config =
            serde_json::from_str(r#"{"bufsize":999999999999,"timeout":99999,"newline":"XX"}"#)
                .unwrap();
        let c = c.sanitized();
        assert_eq!(c.chunk_max(), CHUNK_MAX);
        assert_eq!(c.timeout, TIMEOUT_DEFAULT);
        assert_eq!(c.newline, None, "모르는 줄바꿈은 없는 것으로 친다");
    }

    /// 실서버 회귀(2026-08-21): 파이썬 trzsz는 CFG에 newline을 싣지 않는다.
    /// 그때 협상해 둔 `!\n`을 잃으면 Windows 원격이 우리 답을 못 읽어 전송이 멈춘다.
    #[test]
    fn a_config_without_newline_keeps_the_negotiated_one() {
        let c: Config = serde_json::from_str(r#"{"lang":"py","protocol":1}"#).unwrap();
        assert_eq!(c.sanitized().newline, None);
        let c: Config = serde_json::from_str(r#"{"newline":"!\n"}"#).unwrap();
        assert_eq!(c.sanitized().newline.as_deref(), Some("!\n"));
    }

    #[test]
    fn reads_a_real_looking_config() {
        let c: Config = serde_json::from_str(
            r#"{"lang":"go","bufsize":10485760,"timeout":100,"protocol":1,"client_files":["/a"]}"#,
        )
        .unwrap();
        let c = c.sanitized();
        assert_eq!(c.chunk_max(), CHUNK_MAX);
        assert_eq!(c.client_files, vec!["/a".to_string()]);
    }
}
