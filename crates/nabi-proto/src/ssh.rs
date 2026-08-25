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
    /// ssh-agent 포워딩(OpenSSH `ForwardAgent`). 켜면 **그 서버가 세션 동안 내 키로 서명을
    /// 시킬 수 있다** — 키를 가져가지는 못하지만 그 시간 동안은 나인 척할 수 있다.
    /// 그래서 전역이 아니라 세션마다 켠다(믿는 서버에만).
    #[serde(default)]
    pub agent_forward: bool,
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
            agent_forward: false,
        }
    }

    /// 점프 호스트를 설정한다(ProxyJump). target은 jump를 경유해 연결된다.
    pub fn with_jump(mut self, jump: SshParams) -> Self {
        self.jump = Some(Box::new(jump));
        self
    }

    /// 실행 중인 ssh-agent의 키로 인증(키 파일·비밀번호 없이).
    pub fn agent(host: impl Into<String>, port: u16, user: impl Into<String>) -> Self {
        Self { host: host.into(), port, user: user.into(), auth: SshAuth::Agent, jump: None, agent_forward: false }
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
            agent_forward: false,
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
    /// 실행 중인 ssh-agent(OpenSSH 파이프 또는 Pageant)의 키로 인증.
    /// 개인키가 우리 프로세스로 넘어오지 않는다 — 서명만 에이전트에 부탁한다.
    Agent,
}
