//! 빠른 연결·창 닫기 확인 i18n 항목 — 카탈로그 크기 규율로 catalog2에서 분리.

pub const CATALOG_CONN: &[(&str, &str, &str, &str)] = &[
    ("qc.title", "Quick Connect (SSH)", "빠른 연결 (SSH)", "クイック接続 (SSH)"),
    ("qc.name", "Name", "이름", "名前"),
    ("qc.folder", "Folder", "폴더", "フォルダ"),
    ("qc.saveconnect", "Save & Connect", "저장 후 연결", "保存して接続"),
    ("qc.savepw", "Save password", "비밀번호 저장", "パスワードを保存"),
    ("qc.withsftp", "Open SFTP too", "SFTP도 함께 열기", "SFTPも開く"),
    ("qc.recent", "Recent…", "최근…", "最近…"),
    ("qc.ftpsession", "FTP session", "FTP 세션", "FTPセッション"),
    ("qc.oncommand", "On-connect cmd", "접속 후 명령", "接続後コマンド"),
    ("qc.host", "Host", "호스트", "ホスト"),
    ("qc.port", "Port", "포트", "ポート"),
    ("qc.user", "User", "사용자", "ユーザー"),
    ("qc.password", "Password", "비밀번호", "パスワード"),
    ("qc.keyfile", "Key file", "키 파일", "鍵ファイル"),
    ("qc.connect", "Connect", "연결", "接続"),
    ("qc.cancel", "Cancel", "취소", "キャンセル"),
    ("close.title", "Close window?", "창을 닫을까요?", "ウィンドウを閉じますか?"),
    ("close.confirm", "Close", "닫기", "閉じる"),
];
