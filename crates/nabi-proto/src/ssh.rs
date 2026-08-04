//! SSH 접속 파라미터(Command가 운반; 세션 저장 시 비밀은 제외).

use serde::{Deserialize, Serialize};

/// SSH 접속 정보.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SshParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// 비밀은 직렬화하지 않는다(세션 파일엔 vault 참조만 — [[ssh-security]]).
    #[serde(skip)]
    pub auth: SshAuth,
    /// 점프 호스트(ProxyJump). Some이면 이 호스트를 경유해 target에 연결한다(다단계는 jump.jump로 중첩).
    #[serde(skip)]
    pub jump: Option<Box<SshParams>>,
}

impl SshParams {
    pub fn password(
        host: impl Into<String>,
        port: u16,
        user: impl Into<String>,
        pw: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            user: user.into(),
            auth: SshAuth::Password(pw.into()),
            jump: None,
        }
    }

    /// 점프 호스트를 설정한다(ProxyJump). target은 jump를 경유해 연결된다.
    pub fn with_jump(mut self, jump: SshParams) -> Self {
        self.jump = Some(Box::new(jump));
        self
    }

    /// 개인키 파일 인증(선택적 passphrase).
    pub fn key_file(
        host: impl Into<String>,
        port: u16,
        user: impl Into<String>,
        path: impl Into<String>,
        passphrase: Option<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            user: user.into(),
            auth: SshAuth::KeyFile {
                path: path.into(),
                passphrase,
            },
            jump: None,
        }
    }
}

/// 인증 방식.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum SshAuth {
    /// 비밀 미지정(직렬화 기본값).
    #[default]
    None,
    Password(String),
    KeyFile {
        path: String,
        passphrase: Option<String>,
    },
}
