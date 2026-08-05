//! 전송 큐 i18n 항목(항목별 일시정지·순서변경·제거) — 카탈로그 크기 규율로 분리.

pub const CATALOG_QUEUE: &[(&str, &str, &str, &str)] = &[
    ("sftp.q.waiting", "waiting", "대기", "待機"),
    ("sftp.q.pause", "Pause this item", "이 항목 일시정지", "この項目を一時停止"),
    ("sftp.q.resume", "Resume this item", "이 항목 재개", "この項目を再開"),
    ("sftp.q.up", "Move up in queue", "큐에서 위로", "キューで上へ"),
    ("sftp.q.down", "Move down in queue", "큐에서 아래로", "キューで下へ"),
    ("sftp.q.remove", "Remove from queue", "큐에서 제거", "キューから削除"),
    ("sftp.q.clear", "Clear finished items", "끝난 항목 비우기", "完了項目をクリア"),
    ("sftp.q.cancelall", "Stop all running transfers", "진행 중 전송 모두 중단", "実行中の転送を全て中止"),
    (
        "sessions.delete.warn",
        "The session's pin and note are removed together. This cannot be undone.",
        "고정과 메모도 함께 지워집니다. 되돌릴 수 없습니다.",
        "ピン留めとメモも一緒に削除されます。元に戻せません。",
    ),
    (
        "settings.maxparallel",
        "Parallel transfers per connection",
        "연결당 동시 전송 수",
        "接続あたりの同時転送数",
    ),
];
