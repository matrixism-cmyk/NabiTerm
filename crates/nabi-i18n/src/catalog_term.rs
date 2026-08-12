//! 터미널 보안·동작 관련 i18n 항목 — catalog3 크기 규율로 분리.
//!
//! 원격이 로컬에 영향을 주는 기능(OSC 52 클립보드 쓰기 등)의 설정 문구가 여기 모인다.

pub const CATALOG_TERM: &[(&str, &str, &str, &str)] = &[
    ("osc52.wrote", "Remote wrote to clipboard", "원격이 클립보드에 썼습니다", "リモートがクリップボードに書き込みました"),
    ("settings.osc52", "Remote clipboard (OSC 52)", "원격 클립보드 쓰기(OSC 52)", "リモートのクリップボード書き込み(OSC 52)"),
    ("settings.osc52.block", "Block", "차단", "ブロック"),
    ("settings.osc52.notify", "Allow + notify", "허용하고 알림", "許可して通知"),
    ("settings.osc52.allow", "Allow silently", "조용히 허용", "通知なしで許可"),
    ("settings.osc52hint",
        "A remote host can replace your clipboard via OSC 52. Useful for yanking over SSH, but anything that can write to the terminal can do it.",
        "원격 호스트는 OSC 52로 내 클립보드를 바꿀 수 있습니다. SSH 너머 복사에 유용하지만, 터미널에 글자를 쓸 수 있는 쪽이면 누구나 할 수 있습니다.",
        "リモートホストはOSC 52でクリップボードを書き換えられます。SSH越しのコピーに便利ですが、端末に出力できる相手なら誰でも実行できます。"),
    ("tab.wheelkeys", "Send wheel as page keys", "휠을 페이지 키로 보내기", "ホイールをページキーとして送る"),
    (
        "tab.wheelkeys.hint",
        "For full-screen TUIs that keep their own history and leave the terminal scrollback empty (e.g. codex CLI): the wheel sends PageUp/PageDown to the app instead. Shift+wheel still scrolls the terminal.",
        "자체 화면만 다시 그려 터미널 스크롤백을 비워 두는 TUI(예: codex CLI)용입니다. 휠이 앱에 PageUp/PageDown을 보냅니다. Shift+휠은 그대로 터미널 스크롤백을 봅니다.",
        "自前の画面を描き直して端末のスクロールバックを残さない TUI(例: codex CLI)向けです。ホイールがアプリに PageUp/PageDown を送ります。Shift+ホイールは端末側をスクロールします。",
    ),
];
