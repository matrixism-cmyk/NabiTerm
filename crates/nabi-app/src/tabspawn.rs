//! 로컬 셸 스폰 + 스니펫 가져오기(tabops.rs 라인 한도 분리). 메뉴·팔레트·워크스페이스 공용.

use crate::app::NabiApp;

/// 탐색기 "여기서 열기"로 뜬 경우 첫 셸이 열릴 폴더. **한 번만** 쓴다.
///
/// 환경 변수로 넘어온다(GUI가 뜨기 전에 정해지므로 설정보다 앞선다). 두 번째 탭부터는
/// 평소 규칙(포커스 pane의 cwd → 기본 시작 폴더)을 따라야 하므로 읽고 나서 지운다.
fn take_start_cwd() -> Option<String> {
    let v = std::env::var("NABI_START_CWD").ok().filter(|s| !s.is_empty())?;
    std::env::remove_var("NABI_START_CWD");
    std::path::Path::new(&v).is_dir().then_some(v)
}

impl NabiApp {
    /// 텍스트 파일(줄당 1개)에서 스니펫을 가져온다(기존과 중복 제거 후 추가, 영속 저장).
    pub(crate) fn import_snippets(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("snippets", &["txt"]).pick_file() else { return };
        let Ok(text) = std::fs::read_to_string(&p) else { return };
        let mut added = 0usize;
        for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
            if !self.config.terminal.snippets.iter().any(|s| s == line) {
                self.config.terminal.snippets.push(line.to_string());
                added += 1;
            }
        }
        let _ = nabi_config::save(&self.config_path, &self.config);
        self.notify = Some((format!("{} +{added}", nabi_i18n::tr(self.lang, "menu.importsnippets")), std::time::Instant::now()));
    }

    /// 새 로컬 셸을 연다(출처 기록 + 포커스된 로컬 pane의 작업 디렉터리 상속).
    pub(crate) fn spawn_local(&mut self, shell: nabi_proto::ShellKind) {
        self.spawn_local_with(shell, None);
    }

    /// 로컬 셸을 열고 접속 후 자동 명령(on_connect)을 함께 큐잉한다(포커스 pane의 cwd 상속,
    /// 없으면 설정의 기본 시작 디렉터리, 그것도 없으면 시스템 기본).
    pub(crate) fn spawn_local_with(&mut self, shell: nabi_proto::ShellKind, on_connect: Option<String>) {
        let cwd = self.spawn_cwd().or_else(take_start_cwd).or_else(|| {
            let d = &self.config.terminal.default_cwd;
            (!d.is_empty() && std::path::Path::new(d).is_dir()).then(|| d.clone())
        });
        self.spawn_local_cwd(shell, on_connect, cwd);
    }

    /// 지정한 작업 디렉터리로 새 로컬 셸을 연다(브라우저 "여기서 터미널 열기").
    pub(crate) fn spawn_local_at(&mut self, cwd: String) {
        let shell = crate::workspace::shell_from_str(&self.config.terminal.default_shell);
        self.spawn_local_cwd(shell, None, Some(cwd));
    }

    /// 포커스 pane의 현재 디렉터리에서 새 로컬 셸 탭을 연다(없으면 기본 셸). 터미널 공통 "Open here".
    pub(crate) fn spawn_here(&mut self) {
        match self.focused_pane().and_then(|p| self.cwds.get(&p)).map(|c| crate::workspace::strip_uri_slash(c)).filter(|c| !c.is_empty()) {
            Some(c) => self.spawn_local_at(c),
            None => self.spawn_local(crate::workspace::shell_from_str(&self.config.terminal.default_shell)),
        }
    }

    /// 셸·on_connect·작업 디렉터리를 명시해 로컬 pane을 띄운다(공통 경로).
    pub(crate) fn spawn_local_cwd(&mut self, shell: nabi_proto::ShellKind, on_connect: Option<String>, cwd: Option<String>) {
        let origin = nabi_session::SessionKind::Local { shell: crate::workspace::shell_to_str(&shell) };
        let seq = self.register_spawn(origin, on_connect);
        self.orch.send(nabi_proto::Command::SpawnLocalPane {
            shell,
            size: nabi_types::GridSize::default(),
            scrollback: self.config.terminal.scrollback,
            encoding: self.config.terminal.encoding.clone(),
            cwd,
            reply_seq: Some(seq),
        });
    }
}
