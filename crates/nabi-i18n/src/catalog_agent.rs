//! AI 제어 도움말 번역 — catalog3 라인 한도 분리.

pub(crate) const CATALOG_AGENT: &[(&str, &str, &str, &str)] = &[
    ("help.agent.title", "Control by AI (Claude Code, etc.)", "AI(클로드 코드 등)로 제어", "AI(Claude Code 等)で制御"),
    (
        "help.agent.intro",
        "An AI agent running in a pane can drive nabiTerm — open panes, watch output, send input — via `nabi cli`. Give it the guide below.",
        "분할 칸 안에서 실행 중인 AI가 `nabi cli`로 nabiTerm을 제어할 수 있습니다 — 분할 칸 열기·출력 감시·입력 전송. 아래 사용설명을 AI에게 건네세요.",
        "ペイン内で動く AI が `nabi cli` で nabiTerm を制御できます — ペインを開く・出力監視・入力送信。下のガイドを AI に渡してください。",
    ),
    ("help.agent.examples", "Key commands:", "주요 명령:", "主なコマンド:"),
    ("help.cmd.list", "List all panes (id, title, cwd, state)", "모든 분할 칸 목록(id·제목·cwd·상태)", "全ペイン一覧(id・タイトル・cwd・状態)"),
    ("help.cmd.spawn", "Open a new pane (shell/dir/split)", "새 분할 칸 열기(셸·디렉터리·분할)", "新しいペインを開く(シェル・dir・分割)"),
    ("help.cmd.send", "Send input to a pane (end with \\r = Enter)", "분할 칸에 입력 전송(끝에 \\r = 엔터)", "ペインに入力送信(末尾 \\r = Enter)"),
    ("help.cmd.capture", "Read a pane's output (last N lines)", "분할 칸 출력 읽기(최근 N줄)", "ペインの出力を読む(直近 N 行)"),
    ("help.cmd.wait", "Wait until a command finishes", "명령이 끝날 때까지 대기", "コマンド終了まで待機"),
    ("help.cmd.notify", "Desktop notification", "데스크톱 알림", "デスクトップ通知"),
    ("help.cmd.kill", "Close a pane", "분할 칸 닫기", "ペインを閉じる"),
    (
        "help.agent.perm",
        "Permission mode: Settings > Behavior > Agent control (off / ask / on). In 'ask', inspect commands always work; the first act/inject from a pane asks for approval.",
        "권한 모드: 설정 > 동작 > 에이전트 제어(off / ask / on). 'ask'에서 조회 명령은 항상 동작하고, 분할 칸의 첫 동작/주입은 승인을 묻습니다.",
        "権限モード: 設定 > 動作 > エージェント制御(off / ask / on)。'ask' では参照系は常に動作し、最初の操作/注入は承認を求めます。",
    ),
    ("help.agent.copy", "Copy guide", "사용설명 복사", "ガイドをコピー"),
    ("help.agent.save", "Save as .md", "MD로 저장", ".md で保存"),
    ("help.agent.hint", "Paste it to the AI in a pane to teach it how to control nabiTerm.", "분할 칸의 AI에게 붙여넣어 nabiTerm 제어법을 알려주세요.", "ペインの AI に貼り付けて nabiTerm の制御方法を教えてください。"),
    ("help.agent.copied", "Guide copied to clipboard", "사용설명을 클립보드에 복사했습니다", "ガイドをクリップボードにコピーしました"),
    ("help.agent.saved", "Saved:", "저장됨:", "保存しました:"),
];
