//! i18n 카탈로그 7 — 호스트키 변경 경고·압축·세션 로그 자동·커서 상한(2026-08-26 배치 Q부터).
//!
//! catalog5·6이 소프트 한도에 닿아 새 키는 여기에 쌓는다. 카탈로그는 한 파일이 차면
//! 나누고, **나눈 뒤에는 새 키를 새 파일에 넣는다**(catalog6이 적어 둔 규칙 그대로).

/// (키, 영어, 한국어, 일본어) 4-튜플.
pub(crate) const CATALOG7: &[(&str, &str, &str, &str)] = &[
    ("hostkey.changed.title", "WARNING: the server key has changed",
        "경고: 서버 키가 바뀌었습니다", "警告: サーバー鍵が変わりました"),
    ("hostkey.changed.msg", "This server presented a different key than the one you trusted before.",
        "전에 신뢰한 것과 다른 키를 보내왔습니다.",
        "以前信頼した鍵とは異なる鍵を提示しました。"),
    ("hostkey.oldfp", "Known fingerprint", "알던 지문", "既知の指紋"),
    ("blocks.openout", "Open output in editor", "출력을 편집기로 열기", "出力をエディタで開く"),
    ("edit.cursors.capped", "Stopped at 10,000 cursors - more matches remain", "커서 1만 개에서 멈추었습니다 - 남은 일치가 더 있습니다", "カーソル1万で停止 - 一致はまだあります"),
    ("browser.zipmenu", "Archive", "압축", "アーカイブ"),
    ("browser.zipmake", "Compress to zip", "zip으로 묶기", "zipに圧縮"),
    ("browser.zipextract", "Extract here", "풀기", "ここに展開"),
    ("browser.ziptrunc", "stopped at the limit", "상한에서 멈추었음", "上限で停止"),
    ("browser.zipunsafe", "skipped unsafe paths:", "안전하지 않은 경로 건너뜀:", "安全でないパスをスキップ:"),
    ("hostkey.newfp", "New fingerprint", "새 지문", "新しい指紋"),
    ("hostkey.changed.warn",
        "The server may have been rebuilt - or someone may be intercepting this connection. Check the new fingerprint with the server administrator before continuing.",
        "서버를 새로 세웠을 수도, 누군가 중간에서 가로채고 있을 수도 있습니다. 진행 전에 새 지문을 서버 관리자와 대조하세요.",
        "サーバーを再構築した可能性も、誰かが中間で傘取している可能性もあります。続行前に新しい指紋を管理者と照合してください。"),
    ("hostkey.changed.sure", "I verified the new fingerprint with the server administrator",
        "새 지문을 서버 관리자와 대조했습니다",
        "新しい指紋を管理者と照合しました"),
    ("hostkey.changed.accept", "Replace the stored key", "저장된 키를 교체", "保存された鍵を置換"),
    ("settings.autolog", "Log every session", "모든 세션 기록", "全セッションを記録"),
    ("settings.autologhint",
        "New shells and SSH sessions are written to logs/ in the settings folder. Off by default - terminal output can contain passwords.",
        "새 셀·SSH 창의 출력을 설정 폴더의 logs/에 기록합니다. 기본은 꺼짐 - 터미널 출력에는 비밀번호도 지나갑니다.",
        "新しいシェルやSSHの出力を設定フォルダのlogs/に記録します。既定はオフ - 端末出力にはパスワードも含まれます。"),
];
