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
    ("tab.wheelkeys", "Wheel scrolls TUI history", "휠로 TUI 기록 스크롤", "ホイールでTUI履歴をスクロール"),
    (
        "tab.wheelkeys.hint",
        "For TUIs that keep history to themselves (e.g. codex CLI): wheel-up opens its transcript view (Ctrl+T) and further wheel scrolls it with PageUp/PageDown. Close with Esc or q. Shift+wheel still scrolls the terminal.",
        "기록을 자기 안에만 두는 TUI(예: codex CLI)용입니다. 휠을 위로 굴리면 전사 화면(Ctrl+T)을 열고, 이어지는 휠이 PageUp/PageDown으로 그 안을 스크롤합니다. Esc나 q로 닫습니다. Shift+휠은 그대로 터미널 스크롤백을 봅니다.",
        "履歴を自分だけで持つ TUI(例: codex CLI)向けです。上へ回すとトランスクリプト画面(Ctrl+T)を開き、続くホイールが PageUp/PageDown でスクロールします。Esc か q で閉じます。Shift+ホイールは端末側をスクロールします。",
    ),
    ("settings.resetdefault", "Reset to default", "기본값으로 되돌리기", "既定値に戻す"),
    ("ai.state.done", "Finished (unseen)", "완료(미확인)", "完了(未確認)"),
    (
        "tg.ownerhint",
        "The first chat ID is the owner - only the owner can type into shells or send Ctrl+C. Others can watch (/panes, /use).",
        "첫 번째 chat ID가 오너입니다 — 셸 입력·Ctrl+C는 오너만 가능하고, 나머지는 관찰(/panes·/use)만 됩니다.",
        "先頭の chat ID がオーナーです — シェル入力・Ctrl+C はオーナーのみ、他は閲覧(/panes・/use)のみです。",
    ),
    ("tg.dmpolicy", "Unknown DMs", "미지 DM 처리", "未知のDM"),
    ("tg.dmpolicy.allowlist", "Ignore (allowlist only)", "무시(허용 목록만)", "無視(許可リストのみ)"),
    ("tg.dmpolicy.pairing", "Pairing (code + approval)", "페어링(코드+승인)", "ペアリング(コード+承認)"),
    ("tg.pending", "Pairing requests", "페어링 대기", "ペアリング待ち"),
    ("tg.approve", "Approve", "승인", "承認"),
    ("tg.deny", "Deny", "거부", "拒否"),
];
