//! 네트워크 계층(ssh/sftp/ftp/포워딩) 에러·알림 문구(T8-1).
//!
//! 원산지(nabi-ssh 등)가 `trc()`로 현재 언어 문자열을 만든다 — 이전엔 한국어 하드코딩이라
//! 영/일 사용자 상태바에 한국어 에러가 그대로 노출됐다. `{x}` 자리는 호출부가 replace.

pub(crate) const CATALOG_NET: &[(&str, &str, &str, &str)] = &[
    ("net.agent.none", "No usable ssh-agent (not running, or no keys loaded)", "쓸 수 있는 ssh-agent가 없습니다(에이전트 미실행 또는 등록된 키 없음)", "使用可能なssh-agentがありません(未起動または鍵未登録)"),
    ("net.agent.rejectedmax", "All {n} agent keys were rejected (stopping before the server's MaxAuthTries). Remove unused keys from the agent or specify a key file", "에이전트 키 {n}개가 모두 거부됐습니다(서버 MaxAuthTries 때문에 더 시도하지 않습니다). 쓰지 않는 키를 에이전트에서 빼거나 키 파일을 직접 지정하세요", "エージェントの鍵{n}個がすべて拒否されました(サーバーのMaxAuthTriesのため中断)。不要な鍵を外すか鍵ファイルを指定してください"),
    ("net.agent.rejected", "The server rejected all {n} agent keys", "에이전트의 키 {n}개를 서버가 모두 거부했습니다", "サーバーがエージェントの鍵{n}個をすべて拒否しました"),
    ("net.chan.closed", "ssh channel closed", "ssh 채널 닫힘", "sshチャネルが閉じられました"),
    ("net.ssh.err", "ssh error", "ssh 오류", "sshエラー"),
    ("net.legacy.notice", "Note: connected with legacy (SHA-1) algorithms because the server is outdated", "알림: 서버가 오래되어 레거시 알고리즘(SHA-1)으로 접속했습니다", "注意: サーバーが古いためレガシー(SHA-1)アルゴリズムで接続しました"),
    ("net.pq.notpq", "Note: this connection is NOT quantum-resistant (the server does not offer ML-KEM)", "알림: 이 연결은 양자내성이 아닙니다(서버가 ML-KEM을 제공하지 않습니다)", "注意: この接続は耐量子ではありません(サーバーが ML-KEM を提供していません)"),
    ("net.pq.rejected", "Refused: quantum-resistant key exchange is required, but the server does not offer it", "거부: 양자내성 키 교환을 요구하도록 설정돼 있는데 서버가 제공하지 않습니다", "拒否: 耐量子鍵交換を要求する設定ですが、サーバーが提供していません"),
    ("net.key.load", "Failed to load key", "키 로드 실패", "鍵の読み込みに失敗"),
    ("net.auth.fail", "SSH authentication failed", "SSH 인증 실패", "SSH認証に失敗しました"),
    ("net.fwd.agent", "Forwarding agent auth", "포워딩 에이전트 인증", "フォワーディングのエージェント認証"),
    ("net.fwd.noauth", "Forwarding: no credentials", "포워딩: 인증 정보가 없습니다", "フォワーディング: 認証情報がありません"),
    ("net.fwd.pwonly", "Forwarding: only password auth is supported", "포워딩: 비밀번호 인증만 지원", "フォワーディング: パスワード認証のみ対応"),
    ("net.jump.authfail", "ProxyJump target authentication failed", "ProxyJump 타겟 인증 실패", "ProxyJump先の認証に失敗しました"),
    ("net.jump.pwonly", "ProxyJump: only password auth is supported", "ProxyJump: 비밀번호 인증만 지원", "ProxyJump: パスワード認証のみ対応"),
    ("net.socks.atyp", "SOCKS5: unsupported address type", "SOCKS5 atyp 미지원", "SOCKS5: 未対応のアドレス種別"),
    ("net.x11.pwonly", "X11: only password auth is supported", "X11: 비밀번호 인증만 지원", "X11: パスワード認証のみ対応"),
    ("net.sftp.rename", "posix-rename: unexpected response", "posix-rename: 예기치 않은 응답", "posix-rename: 予期しない応答"),
    ("net.sftp.agent", "SFTP agent auth", "SFTP 에이전트 인증", "SFTPのエージェント認証"),
    ("net.sftp.noauth", "SFTP: no credentials", "SFTP: 인증 정보가 없습니다", "SFTP: 認証情報がありません"),
    ("net.sftp.authfail", "SFTP authentication failed", "SFTP 인증 실패", "SFTP認証に失敗しました"),
    ("net.xfer.canceled", "Transfer canceled", "전송 취소됨", "転送がキャンセルされました"),
    ("net.xfer.dlsize", "Download size mismatch: {a}/{b} bytes", "다운로드 크기 불일치: {a}/{b} 바이트", "ダウンロードサイズ不一致: {a}/{b} バイト"),
    ("net.xfer.dlhash", "Download hash mismatch (SHA-256): local {l} ≠ remote {r}", "다운로드 해시 불일치(SHA-256): 로컬 {l} ≠ 원격 {r}", "ダウンロードのハッシュ不一致(SHA-256): ローカル {l} ≠ リモート {r}"),
    ("net.xfer.upsize", "Upload size mismatch: {a}/{b} bytes", "업로드 크기 불일치: {a}/{b} 바이트", "アップロードサイズ不一致: {a}/{b} バイト"),
    ("net.xfer.uphash", "Upload hash mismatch (SHA-256): local {l} ≠ remote {r}", "업로드 해시 불일치(SHA-256): 로컬 {l} ≠ 원격 {r}", "アップロードのハッシュ不一致(SHA-256): ローカル {l} ≠ リモート {r}"),
    ("net.xfer.nospace", "Not enough free space on remote: {free} < {need} bytes", "원격 여유 공간 부족: {free} < {need} 바이트", "リモートの空き容量不足: {free} < {need} バイト"),
    ("net.ftp.timeout", "Connection timed out", "연결 시간 초과", "接続タイムアウト"),
];
