//! i18n 카탈로그 5 — 붙여넣기 안전·설정 재편·에디터 명령(2026-08-19 배치부터).
//!
//! catalog4가 소프트 라인 한도에 닿아 새 키는 여기에 쌓는다.

/// (키, 영어, 한국어, 일본어) 4-튜플.
pub(crate) const CATALOG5: &[(&str, &str, &str, &str)] = &[
    // 붙여넣기 유니코드 속임 경고(T1-5).
    ("paste.risk.title", "This paste contains deceptive characters", "붙여넣기에 속임 문자가 섞여 있습니다", "貼り付けに欺瞞的な文字が含まれています"),
    ("paste.risk.bidi", "Text direction override — what you see is not the real order", "방향 재정의 — 화면에 보이는 순서와 실제 순서가 다릅니다", "方向の上書き — 表示順と実際の順序が異なります"),
    ("paste.risk.zerowidth", "Zero-width characters hidden in the text", "본문에 숨은 제로폭(보이지 않는) 문자", "本文に隠れたゼロ幅(不可視)文字"),
    ("paste.risk.mixedscript", "A word mixes Latin with Cyrillic/Greek look-alikes", "한 낱말에 라틴과 키릴·그리스 유사 글자가 섞임", "1つの語にラテンとキリル・ギリシャの類似字が混在"),
    ("paste.risk.oddspace", "Unusual Unicode spaces (not a plain space)", "보통 공백이 아닌 유니코드 공백", "通常の空白ではないUnicode空白"),
    ("paste.strip", "Remove and paste", "위험 문자 제거 후 붙여넣기", "危険な文字を除いて貼り付け"),
    ("settings.warnpasteunicode", "Warn on deceptive Unicode in paste", "붙여넣기 속임 문자 경고", "貼り付けの欺瞞文字を警告"),
    ("settings.uploadmode", "Uploaded file permissions", "업로드 파일 권한", "アップロード時の権限"),
    ("settings.uploadmodehint", "Empty = leave to the server. auto = 644, scripts 755. Or an octal mode like 644.", "비우면 서버 기본값. auto=일반 644·스크립트 755. 또는 644 같은 8진수.", "空ならサーバー既定。auto=通常644・スクリプト755。または644などの8進数。"),
    ("editor.keepdup", "Keep only duplicate lines", "중복 줄만 남기기", "重複行のみ残す"),
    ("editor.duplines", "Duplicate each line", "줄 복제", "各行を複製"),
    ("editor.wrapcol", "Hard wrap width", "줄바꿈 폭", "ハードラップ幅"),
    ("paste.title.one", "Paste this?", "붙여넣을까요?", "貼り付けますか?"),
    // 설정 페이지 통합(사용자 요청 2026-08-21): 모양 4→1, 원격 2→1, AI 터미널은 전용 창으로.
    ("settings.sec.appearance", "Appearance", "모양", "外観"),
    ("settings.sec.remote", "Remote (SSH / SFTP)", "원격 연결", "リモート接続"),
    // Antigravity CLI(agy) — Gemini CLI(2026-06-18 종료) 후속. 명령은 공식 레퍼런스 기준.
    ("aicb.l.agents", "Agents", "에이전트", "エージェント"),
    ("aicb.l.perms", "Permissions", "권한", "権限"),
    ("aicb.l.tasks", "Tasks", "작업", "タスク"),
    ("aicb.agy.clear", "Clear the terminal and reset the conversation", "화면을 지우고 대화를 초기화", "画面を消去し会話をリセット"),
    ("aicb.agy.context", "Show context usage", "컨텍스트 사용량 보기", "コンテキスト使用量を表示"),
    ("aicb.agy.usage", "Show model quota usage", "모델 사용량(할당량) 보기", "モデル使用量(クォータ)を表示"),
    ("aicb.agy.model", "Choose the reasoning model", "추론 모델 선택", "推論モデルを選択"),
    ("aicb.agy.resume", "Pick a previous conversation to resume", "이전 대화 골라 이어가기", "以前の会話を選んで再開"),
    ("aicb.agy.diff", "Open the interactive diff viewer", "변경 내용 보기(diff)", "変更内容を表示(diff)"),
    ("aicb.agy.agents", "Switch agents and watch background subagents", "에이전트 전환·백그라운드 서브에이전트 확인", "エージェント切替・バックグラウンド確認"),
    ("aicb.agy.perms", "Manage tool permissions", "도구 권한 관리", "ツール権限の管理"),
    ("aicb.agy.skills", "Browse loaded Agent Skills", "불러온 에이전트 스킬 보기", "読み込んだエージェントスキルを表示"),
    ("aicb.agy.mcp", "Open the MCP server manager", "MCP 서버 관리자 열기", "MCPサーバー管理を開く"),
    ("aicb.agy.tasks", "Watch background shell tasks", "백그라운드 작업 로그 보기", "バックグラウンド作業ログを表示"),
    ("aicb.agy.rewind", "Roll the conversation back to an earlier message", "대화를 이전 지점으로 되돌리기", "会話を以前の時点に戻す"),
    ("aicb.agy.copy", "Copy the last response to the clipboard", "마지막 답변 클립보드 복사", "最後の応答をクリップボードにコピー"),
    ("aicb.agy.config", "Open the settings editor", "설정 편집기 열기", "設定エディタを開く"),
    ("aiopt.agy.skipperm", "Run every tool call without asking (dangerous)", "모든 도구 호출을 묻지 않고 실행(위험)", "すべてのツール呼出を確認なしで実行(危険)"),
    ("aiopt.agy.sandbox", "Run the session sandboxed", "샌드박스로 실행", "サンドボックスで実行"),
    // 메뉴 띠 업데이트 버튼 + 재시작 안내(사용자 요청 2026-08-21).
    ("update.btn", "Update", "업데이트", "アップデート"),
    ("update.btn.hint", "A new version is ready - see what changed and install", "새 버전이 준비됐습니다 - 변경 내용을 보고 설치", "新しいバージョンがあります - 変更内容を確認して更新"),
    ("update.restartwarn", "nabiTerm will close and reopen to finish the update. Save your work first.", "업데이트하면 nabiTerm이 종료됐다가 다시 실행됩니다. 작업을 먼저 저장하세요.", "更新するとnabiTermは終了して再起動します。作業を先に保存してください。"),
    // 세션 다중 선택·일괄 연결(실사용자 피드백 2026-08-21).
    ("bulk.connect", "Connect selected", "선택 연결", "選択を接続"),
    ("bulk.clear", "Clear selection", "선택 해제", "選択を解除"),
    ("bulk.title", "Connect selected sessions", "선택한 세션 연결", "選択したセッションを接続"),
    ("bulk.count", "Selected", "선택", "選択"),
    ("bulk.auto", "Connect automatically (vault / key)", "자동 연결(볼트·키)", "自動接続(ボルト・鍵)"),
    ("bulk.needlogin", "Need a password (one dialog each)", "비밀번호 입력 필요(각각 창이 뜸)", "パスワード入力が必要(それぞれ画面が出ます)"),
    ("bulk.hint", "Sessions without saved credentials open a connect dialog one by one.", "자격증명이 저장되지 않은 세션은 접속 창이 하나씩 뜹니다.", "資格情報が保存されていないセッションは接続画面が個別に開きます。"),
    ("bulk.onlyready", "Connect only the automatic ones", "자동 연결되는 것만", "自動接続のみ"),
    ("bulk.all", "Connect all", "전부 연결", "すべて接続"),
    ("bulk.pickmode", "Select", "선택 모드", "選択モード"),
    ("bulk.pickmode.hint", "Click rows to select instead of connecting (Ctrl/Shift also work)", "클릭이 연결 대신 선택이 됩니다(Ctrl·Shift도 동작)", "クリックが接続ではなく選択になります(Ctrl・Shiftも有効)"),
];
