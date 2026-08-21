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
];
