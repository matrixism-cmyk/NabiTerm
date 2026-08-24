//! 저장 세션 도메인 타입(비밀 없음).

use serde::{Deserialize, Serialize};

/// 세션 종류.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionKind {
    /// 로컬 셸(프로그램 이름).
    Local { shell: String },
    /// SSH(자격증명은 vault 키로만 참조).
    Ssh {
        host: String,
        port: u16,
        user: String,
        /// 비밀 핸들(volt 키). 비밀 자체는 저장하지 않는다.
        credential_ref: Option<String>,
        /// 개인키 파일 경로(선택, 비밀 아님).
        #[serde(default)]
        key_path: Option<String>,
        /// 점프 호스트(ProxyJump, "user@bastion:22"). 비우면 직접 연결. 비밀 아님(B4).
        #[serde(default)]
        jump: Option<String>,
    },
}

/// 저장된 한 세션 항목.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedSession {
    pub name: String,
    pub folder: Option<String>,
    pub kind: SessionKind,
    /// 접속 직후 자동 실행할 명령(비우면 없음). 워크스페이스 복원 시 종료 직전
    /// 실행 중이던 명령(claude 등)을 재실행하는 데도 쓰인다.
    #[serde(default)]
    pub on_connect: Option<String>,
    /// 로컬 셸 시작 디렉터리(워크스페이스 복원 시 마지막 cwd). 비우면 기본.
    #[serde(default)]
    pub cwd: Option<String>,
    /// true면 SSH 터미널이 아니라 FTP 파일 브라우저로 연다(kind는 Ssh 재사용).
    #[serde(default)]
    pub is_ftp: bool,
    /// true면 SSH 터미널 연결과 동시에 SFTP 브라우저도 함께 연다(저장 세션 클릭 시).
    #[serde(default)]
    pub open_sftp: bool,
    /// 이 세션의 표식(운영/스테이징/개발…). 사이드바·탭·상태바에 색으로 나타난다.
    ///
    /// **연결하기 전부터 보여야** 뜻이 있다 — 운영 서버라는 것을 알고 눌러야 하기 때문이다.
    /// 그래서 실행 중 탭 색(`app.tab_colors`)과 달리 세션에 저장한다.
    #[serde(default)]
    pub tag: SessionTag,
}

/// 세션 표식. 색과 뜻을 함께 묶는다 — 색만 고르게 하면 사람마다 뜻이 달라진다.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionTag {
    /// 표식 없음(기존 세션의 기본값).
    #[default]
    None,
    /// 운영 — 되돌릴 수 없는 일이 일어나는 곳.
    Prod,
    /// 스테이징.
    Staging,
    /// 개발·시험.
    Dev,
    /// 개인 표식(뜻은 사용자가 정한다).
    Note,
}

impl SessionTag {
    /// 화면에 쓸 색 (r, g, b). 색만으로 구분하지 않도록 라벨과 늘 함께 쓴다(색각 이상 배려).
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            SessionTag::None => (120, 130, 145),
            SessionTag::Prod => (214, 72, 72),
            SessionTag::Staging => (222, 158, 54),
            SessionTag::Dev => (86, 166, 106),
            SessionTag::Note => (108, 140, 214),
        }
    }

    /// i18n 키(라벨).
    pub fn key(self) -> &'static str {
        match self {
            SessionTag::None => "tag.none",
            SessionTag::Prod => "tag.prod",
            SessionTag::Staging => "tag.staging",
            SessionTag::Dev => "tag.dev",
            SessionTag::Note => "tag.note",
        }
    }

    /// **되돌릴 수 없는 곳인가.** 붙여넣기·일괄 실행 전에 한 번 더 묻는 기준이다.
    pub fn is_risky(self) -> bool {
        matches!(self, SessionTag::Prod)
    }

    /// 고를 수 있는 전체 목록(설정 UI·컨텍스트 메뉴가 공유 — SSOT).
    pub const ALL: [SessionTag; 5] =
        [SessionTag::None, SessionTag::Prod, SessionTag::Staging, SessionTag::Dev, SessionTag::Note];
}

impl SavedSession {
    /// 표시용 연결 대상: SSH는 `user@host[:port]`(user 없으면 host, 22는 포트 생략), 로컬은 셸 이름.
    /// conn_hint·SSH URL 등 여러 표시 지점이 이 한 함수를 공유한다(SSOT).
    pub fn target_string(&self) -> String {
        match &self.kind {
            SessionKind::Ssh { host, port, user, .. } => {
                let h = if *port == 22 { host.clone() } else { format!("{host}:{port}") };
                if user.is_empty() {
                    h
                } else {
                    format!("{user}@{h}")
                }
            }
            SessionKind::Local { shell } => shell.clone(),
        }
    }

    /// 종류 라벨(표시 배지용): "FTP" / "SSH" / "Local".
    pub fn kind_label(&self) -> &'static str {
        match &self.kind {
            SessionKind::Ssh { .. } if self.is_ftp => "FTP",
            SessionKind::Ssh { .. } => "SSH",
            SessionKind::Local { .. } => "Local",
        }
    }
}

/// 세션이 질의(대소문자 무시)에 매칭되는지 — 이름/폴더/호스트/사용자를 통합 검색.
/// 빈 질의는 항상 true. 세션 트리 필터와 메뉴 검색바가 같은 기준을 쓰도록 공용으로 둔다.
pub fn session_matches(s: &SavedSession, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let mut t = format!("{} {}", s.name, s.folder.as_deref().unwrap_or(""));
    if let SessionKind::Ssh { host, user, .. } = &s.kind {
        t.push_str(&format!(" {host} {user}"));
    }
    let t = t.to_lowercase();
    // 공백 분리 토큰 전부 매칭(AND, 순서 무관) — "web prod"로 "prod-web"도 검색.
    q.split_whitespace().all(|w| t.contains(w))
}

/// 저장 세션 트리.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionTree {
    pub sessions: Vec<SavedSession>,
}

impl SessionTree {
    pub fn add(&mut self, session: SavedSession) {
        self.sessions.push(session);
    }

    pub fn remove(&mut self, name: &str) {
        self.sessions.retain(|s| s.name != name);
    }

    /// 폴더 → 이름순(대소문자 무시)으로 안정 정렬한다(MobaXterm식 정리). 폴더 없는 항목이 먼저.
    pub fn sort(&mut self) {
        self.sessions.sort_by(|a, b| {
            let fa = a.folder.as_deref().unwrap_or("").to_lowercase();
            let fb = b.folder.as_deref().unwrap_or("").to_lowercase();
            fa.cmp(&fb).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }

    /// 세션 이름을 바꾼다(새 이름이 이미 있으면 거부). 성공 여부를 돌려준다.
    pub fn rename(&mut self, old: &str, new: &str) -> bool {
        let new = new.trim();
        if new.is_empty() || old == new || self.sessions.iter().any(|s| s.name == new) {
            return false;
        }
        match self.sessions.iter_mut().find(|s| s.name == old) {
            Some(s) => {
                s.name = new.to_string();
                true
            }
            None => false,
        }
    }

    /// 세션을 다른 폴더로 옮긴다(None=폴더 없음). 성공 여부를 돌려준다.
    pub fn move_to_folder(&mut self, name: &str, folder: Option<String>) -> bool {
        match self.sessions.iter_mut().find(|s| s.name == name) {
            Some(s) => {
                s.folder = folder.filter(|f| !f.trim().is_empty());
                true
            }
            None => false,
        }
    }

    /// 호스트(SSH)/셸(로컬)순 → 이름순으로 안정 정렬한다(대소문자 무시).
    pub fn sort_by_host(&mut self) {
        let keyof = |s: &SavedSession| match &s.kind {
            SessionKind::Ssh { host, .. } => host.to_lowercase(),
            SessionKind::Local { shell } => shell.to_lowercase(),
        };
        self.sessions.sort_by(|a, b| keyof(a).cmp(&keyof(b)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    }

    /// 폴더별로 묶는다(폴더 없는 그룹 먼저, 폴더는 사전순). 그룹 내 순서는 원본 유지.
    pub fn group_by_folder(&self) -> Vec<(Option<String>, Vec<&SavedSession>)> {
        let mut order: Vec<Option<String>> = vec![None];
        order.extend(self.folders().into_iter().map(Some));
        order
            .into_iter()
            .map(|f| {
                let items: Vec<&SavedSession> = self.sessions.iter().filter(|s| s.folder == f).collect();
                (f, items)
            })
            .filter(|(_, items)| !items.is_empty())
            .collect()
    }

    /// 한 폴더의 모든 세션을 삭제한다. 삭제된 수를 돌려준다.
    pub fn remove_folder(&mut self, name: &str) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.folder.as_deref() != Some(name));
        before - self.sessions.len()
    }

    /// 폴더 이름을 일괄 변경한다(빈 새 이름이면 폴더 해제). 변경된 세션 수를 돌려준다.
    pub fn rename_folder(&mut self, old: &str, new: &str) -> usize {
        let new = new.trim();
        let target = (!new.is_empty()).then(|| new.to_string());
        let mut n = 0;
        for s in self.sessions.iter_mut().filter(|s| s.folder.as_deref() == Some(old)) {
            s.folder = target.clone();
            n += 1;
        }
        n
    }

    /// 이름으로 세션을 찾는다.
    pub fn find(&self, name: &str) -> Option<&SavedSession> {
        self.sessions.iter().find(|s| s.name == name)
    }

    /// 복제용으로 충돌하지 않는 이름을 만든다("base copy", "base copy 2", …).
    pub fn unique_copy_name(&self, base: &str) -> String {
        let mut name = format!("{base} copy");
        let mut n = 2;
        while self.sessions.iter().any(|x| x.name == name) {
            name = format!("{base} copy {n}");
            n += 1;
        }
        name
    }

    /// (SSH 세션 수, 로컬 세션 수) — 상태 표시용.
    pub fn ssh_count(&self) -> (usize, usize) {
        let ssh = self.sessions.iter().filter(|s| matches!(s.kind, SessionKind::Ssh { .. })).count();
        (ssh, self.sessions.len() - ssh)
    }

    /// 같은 호스트(SSH)의 세션들을 찾는다(대소문자 무시).
    pub fn find_by_host(&self, host: &str) -> Vec<&SavedSession> {
        let h = host.to_lowercase();
        self.sessions
            .iter()
            .filter(|s| matches!(&s.kind, SessionKind::Ssh { host, .. } if host.to_lowercase() == h))
            .collect()
    }

    /// 다른 트리를 병합한다(같은 이름은 덮어쓰기, 그 후 같은 대상 중복 제거). 병합 후 추가된 순증을 돌려준다.
    pub fn merge(&mut self, other: SessionTree) -> usize {
        let before = self.sessions.len();
        for s in other.sessions {
            self.remove(&s.name);
            self.add(s);
        }
        self.dedup();
        self.sessions.len().saturating_sub(before)
    }

    /// 한 폴더의 모든 세션을 다른 폴더로 옮긴다(None=폴더 해제). 옮긴 수를 돌려준다.
    pub fn move_all_to_folder(&mut self, from: Option<&str>, to: Option<String>) -> usize {
        let to = to.filter(|f| !f.trim().is_empty());
        let mut n = 0;
        for s in self.sessions.iter_mut().filter(|s| s.folder.as_deref() == from) {
            s.folder = to.clone();
            n += 1;
        }
        n
    }

    /// 질의(대소문자 무시)가 이름/호스트/사용자/폴더에 포함되는 세션만 추린다(검색바용).
    /// 빈 질의는 전체를 돌려준다. 매칭 판정은 [`session_matches`]를 공유(메뉴 UI와 동일 기준).
    pub fn filter(&self, query: &str) -> Vec<&SavedSession> {
        self.sessions.iter().filter(|s| session_matches(s, query)).collect()
    }

    /// 사용 중인 SSH 호스트 목록(중복 제거, 사전순). 퀵커넥트 자동완성 등에 활용.
    pub fn hosts(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .sessions
            .iter()
            .filter_map(|s| match &s.kind {
                SessionKind::Ssh { host, .. } => Some(host.clone()),
                _ => None,
            })
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// 사용 중인 폴더 이름 목록(중복 제거, 사전순).
    pub fn folders(&self) -> Vec<String> {
        let mut v: Vec<String> = self.sessions.iter().filter_map(|s| s.folder.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// 같은 접속 대상(SSH host/port/user/ftp 또는 Local shell) 중복을 제거한다(첫 항목 유지).
    /// 제거한 개수를 돌려준다.
    pub fn dedup(&mut self) -> usize {
        let before = self.sessions.len();
        let mut seen = std::collections::HashSet::new();
        self.sessions.retain(|s| {
            let key = match &s.kind {
                SessionKind::Ssh { host, port, user, .. } => format!("s:{host}:{port}:{user}:{}", s.is_ftp),
                SessionKind::Local { shell } => format!("l:{shell}"),
            };
            seen.insert(key)
        });
        before - self.sessions.len()
    }
}
