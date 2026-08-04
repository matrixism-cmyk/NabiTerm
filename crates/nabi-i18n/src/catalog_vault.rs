//! 볼트(자격증명 금고) 관련 i18n 항목 — 카탈로그 크기 규율로 catalog에서 분리.

pub const CATALOG_VAULT: &[(&str, &str, &str, &str)] = &[
    ("vault.title", "Unlock Vault", "볼트 잠금 해제", "Vault のロック解除"),
    ("vault.prompt", "Master password:", "마스터 비밀번호:", "マスターパスワード:"),
    ("vault.unlock", "Unlock", "잠금 해제", "ロック解除"),
    ("vault.create", "Create Vault", "볼트 만들기", "Vault 作成"),
    ("vault.unlocked", "Vault unlocked. Stored entries:", "볼트 잠금 해제됨. 저장 항목:", "解除済み。保存:"),
    ("vault.lock", "Lock", "잠금", "ロック"),
    ("vault.reset", "Reset (delete vault)", "초기화(볼트 삭제)", "リセット(削除)"),
    ("vault.remember", "Remember password (OS credential)", "비밀번호 기억(OS 자격증명)", "パスワードを記憶(OS資格情報)"),
    ("vault.remember.warn", "Stores the master password in Windows Credential Manager and auto-unlocks on start. Convenient but lower security — anyone using this Windows account can open the vault.", "마스터 비밀번호를 Windows 자격증명 관리자에 저장하고 시작 시 자동으로 잠금을 해제합니다. 편리하지만 보안은 낮아집니다 — 이 Windows 계정을 쓰는 누구나 볼트를 열 수 있습니다.", "マスターパスワードをWindows資格情報マネージャーに保存し、起動時に自動でロック解除します。便利ですが安全性は下がります — このWindowsアカウントを使う誰でも開けます。"),
];
