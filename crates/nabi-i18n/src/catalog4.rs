//! 번역 카탈로그 4 — 상용화 계획(Phase 0~4) 신규 표면. catalog3 라인 한도로 분리.

pub(crate) const CATALOG4: &[(&str, &str, &str, &str)] = &[
    ("menu.tools", "Tools", "도구", "ツール"),
    ("snap.save", "Save workspace snapshot…", "워크스페이스 스냅샷 저장…", "ワークスペーススナップショット保存…"), ("snap.list", "Workspace snapshots…", "워크스페이스 스냅샷 목록…", "ワークスペーススナップショット一覧…"),
    ("snap.namehint", "Snapshot name (e.g. deploy day)", "스냅샷 이름(예: 배포날)", "スナップショット名(例: リリース日)"), ("snap.empty", "(no snapshots yet)", "(저장된 스냅샷 없음)", "(スナップショットなし)"),
    ("snap.openhint", "Switch to this snapshot (current state is saved first)", "이 스냅샷으로 전환(현재 상태는 먼저 저장됩니다)", "このスナップショットへ切替(現在の状態は先に保存)"), ("snap.dirty", "Save your edited documents first", "미저장 문서를 먼저 저장하세요", "未保存の文書を先に保存してください"), ("ob.title", "Welcome to nabiTerm", "나비텀에 오신 것을 환영합니다", "nabiTermへようこそ"),
    ("ob.intro", "Pick a few basics to get started. Everything can be changed later in Settings.", "시작하기 전에 몇 가지만 골라 주세요. 모두 나중에 설정에서 바꿀 수 있습니다.", "始める前にいくつか選んでください。すべて後で設定から変更できます。"), ("ob.hint", "Tip: SSH sessions, SFTP and the nabiPad editor are ready in the menus above.", "팁: SSH 세션·SFTP·nabiPad 편집기는 상단 메뉴에 준비되어 있습니다.", "ヒント: SSHセッション・SFTP・nabiPadエディタは上部メニューにあります。"),
    ("ob.lang", "Language", "언어", "言語"), ("ob.shell", "Default shell", "기본 셸", "既定のシェル"), ("ob.font", "Font size", "글꼴 크기", "フォントサイズ"),
    ("ob.start", "Start", "시작하기", "はじめる"), ("settings.verifyhash", "Verify transfers (SHA-256)", "전송 해시 검증(SHA-256)", "転送ハッシュ検証(SHA-256)"),
    ("settings.verifyhashhint", "After each transfer, compare SHA-256 with the server (rclone-style). Skipped silently if the server has no hash command; size check always runs.", "전송이 끝날 때마다 서버와 SHA-256을 대조합니다(rclone 방식). 서버에 해시 명령이 없으면 조용히 건너뛰며, 크기 비교는 항상 수행됩니다.", "転送のたびにサーバーとSHA-256を照合します(rclone方式)。ハッシュコマンドがないサーバーでは自動的にスキップされ、サイズ照合は常に実行されます。"),
    ("status.pq", "Post-quantum key exchange (ML-KEM768+X25519) — protected against harvest-now-decrypt-later attacks", "포스트퀀텀 키 교환(ML-KEM768+X25519) — 지금 수집해 나중에 푸는 공격으로부터 보호됩니다", "ポスト量子鍵交換(ML-KEM768+X25519) — 収集後解読攻撃から保護されます"),
    ("status.kex", "Key exchange", "키 교환", "鍵交換"), ("status.cipher", "Cipher", "암호", "暗号"),
];
