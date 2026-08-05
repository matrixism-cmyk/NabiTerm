//! 오래된 SSH 서버 호환 — 협상 실패 시 한 번만 레거시 알고리즘으로 재시도한다.
//!
//! russh의 기본 목록은 SHA-1 계열을 통째로 뺀다. 그래서 OpenSSH 4.x(CentOS 5 시절) 같은
//! 서버에는 아예 붙지 못한다 — 그쪽이 내놓는 건 `diffie-hellman-group14-sha1`과
//! `hmac-sha1`뿐이다. MobaXterm·PuTTY가 붙는 서버에 우리만 못 붙으면 안 된다.
//!
//! 그렇다고 SHA-1을 늘 켜 두지는 않는다. **먼저 현대 알고리즘으로 시도하고**, 서버가
//! 공통 알고리즘이 없다고 할 때만 레거시 목록으로 한 번 더 붙는다. 최신 서버는 영향이
//! 없고, 옛 서버만 옛 방식으로 붙는다.

use russh::client::Config;
use russh::{cipher, kex, mac, Preferred};
use std::borrow::Cow;
use std::sync::Arc;

/// 연결 설정에서 호출부마다 다른 부분(나머지는 공통).
#[derive(Clone, Copy)]
pub struct ConnOpts {
    /// keepalive 간격(초). 0이면 끄기.
    pub keepalive_secs: u64,
    /// Nagle 끄기(작은 요청이 잦은 SFTP에 유리).
    pub nodelay: bool,
    /// SSH 채널 창 크기(처리량 상한 = 창 ÷ RTT).
    pub window_size: u32,
}

impl Default for ConnOpts {
    fn default() -> Self {
        Self { keepalive_secs: 30, nodelay: false, window_size: 2 * 1024 * 1024 }
    }
}

/// 공통 클라이언트 설정을 만든다. `legacy`면 SHA-1 계열을 뒤에 덧붙인다(선호 순서는 유지).
pub fn make_config(o: &ConnOpts, legacy: bool) -> Arc<Config> {
    let mut c = Config {
        keepalive_interval: (o.keepalive_secs > 0)
            .then(|| std::time::Duration::from_secs(o.keepalive_secs)),
        keepalive_max: 3,
        nodelay: o.nodelay,
        window_size: o.window_size,
        ..Default::default()
    };
    if legacy {
        c.preferred = legacy_preferred();
    }
    Arc::new(c)
}

/// 기본 목록 **뒤에** 레거시 알고리즘을 덧붙인 선호 목록.
///
/// 앞쪽(현대 알고리즘)이 그대로 우선이라, 양쪽을 다 지원하는 서버는 현대 쪽으로 붙는다.
fn legacy_preferred() -> Preferred {
    let d = Preferred::DEFAULT;
    let extend = |base: &[kex::Name], extra: &[kex::Name]| -> Cow<'static, [kex::Name]> {
        Cow::Owned([base, extra].concat())
    };
    Preferred {
        kex: extend(&d.kex, &[kex::DH_GEX_SHA1, kex::DH_G14_SHA1, kex::DH_G1_SHA1]),
        key: d.key,
        // 옛 서버는 CTR을 가진 경우가 많지만 CBC만 있는 것도 있다.
        // (3des-cbc는 russh의 `des` 기능이 필요한데 우리는 기능을 최소로 켠다 — 제외.)
        cipher: Cow::Owned(
            [d.cipher.to_vec(), vec![cipher::AES_256_CBC, cipher::AES_192_CBC, cipher::AES_128_CBC]]
                .concat(),
        ),
        // hmac-sha2-*는 OpenSSH 5.9부터다. 그 이전 서버에는 hmac-sha1밖에 없다.
        mac: Cow::Owned([d.mac.to_vec(), vec![mac::HMAC_SHA1_ETM, mac::HMAC_SHA1]].concat()),
        compression: d.compression,
    }
}

/// "공통 알고리즘이 없다"는 협상 실패인가 — 이때만 레거시로 다시 시도할 값어치가 있다.
///
/// 인증 실패·호스트키 거부·타임아웃은 다시 붙어도 결과가 같으므로 재시도하지 않는다.
pub fn is_algo_mismatch(e: &russh::Error) -> bool {
    matches!(e, russh::Error::NoCommonAlgo { .. })
}

/// 현대 설정으로 먼저 붙고, 공통 알고리즘이 없으면 레거시 설정으로 한 번만 더 붙는다.
///
/// 반환의 `bool`은 레거시로 붙었는지 — 호출부가 사용자에게 알려 줄 수 있게 넘긴다.
pub async fn connect_compat<T, F, Fut>(o: &ConnOpts, mut attempt: F) -> Result<(T, bool), russh::Error>
where
    F: FnMut(Arc<Config>) -> Fut,
    Fut: std::future::Future<Output = Result<T, russh::Error>>,
{
    match attempt(make_config(o, false)).await {
        Ok(v) => Ok((v, false)),
        Err(e) if is_algo_mismatch(&e) => attempt(make_config(o, true)).await.map(|v| (v, true)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 레거시 목록은 기본 목록을 **덮지 않고 뒤에 붙인다** — 최신 서버는 영향이 없어야 한다.
    #[test]
    fn legacy_extends_not_replaces() {
        let d = Preferred::DEFAULT;
        let l = legacy_preferred();
        assert_eq!(&l.kex[..d.kex.len()], &d.kex[..], "앞쪽 선호 순서가 그대로여야 한다");
        assert!(l.kex.contains(&kex::DH_G14_SHA1), "옛 서버용 KEX가 있어야 한다");
        assert!(l.mac.contains(&mac::HMAC_SHA1), "옛 서버용 MAC이 있어야 한다");
        assert!(l.kex.len() > d.kex.len() && l.mac.len() > d.mac.len());
    }

    /// 인증 실패로는 재시도하지 않는다(같은 결과가 나올 뿐이고, 시도 횟수만 축낸다).
    ///
    /// 참을 확인하는 쪽은 여기서 못 쓴다 — `NoCommonAlgo`의 `kind`가 러스에서 비공개라
    /// 값을 만들 수 없다. 그 경로는 실제 옛 서버로 검증한다(`legacy_server_falls_back`).
    #[test]
    fn only_negotiation_failure_retries() {
        assert!(!is_algo_mismatch(&russh::Error::NotAuthenticated));
        assert!(!is_algo_mismatch(&russh::Error::ConnectionTimeout));
        assert!(!is_algo_mismatch(&russh::Error::Disconnect));
    }

    /// 실제 옛 서버(OpenSSH 4.x 등)에 붙어 폴백이 도는지 본다.
    ///
    /// 환경변수로만 돈다: `NABI_OLD_HOST` `NABI_OLD_USER` `NABI_OLD_PASS`(선택 `NABI_OLD_PORT`).
    /// 현대 알고리즘만으로는 **반드시 실패**해야 하고, 폴백으로는 **성공**해야 한다 —
    /// 둘 다 확인해야 "폴백 덕분에 붙었다"고 말할 수 있다.
    #[tokio::test]
    #[ignore = "옛 SSH 서버 필요(NABI_OLD_HOST/USER/PASS)"]
    async fn legacy_server_falls_back() {
        let (Ok(host), Ok(user), Ok(pass)) = (
            std::env::var("NABI_OLD_HOST"),
            std::env::var("NABI_OLD_USER"),
            std::env::var("NABI_OLD_PASS"),
        ) else {
            return;
        };
        let port: u16 =
            std::env::var("NABI_OLD_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);
        let o = ConnOpts::default();
        let kh = std::env::temp_dir().join("nabi-legacy-test-known-hosts");
        let handler =
            || crate::handler::ClientHandler::new(host.clone(), port, kh.clone(), None);
        // 1) 기본(현대) 목록만으로는 협상이 깨져야 한다.
        let modern =
            russh::client::connect(make_config(&o, false), (host.as_str(), port), handler()).await;
        assert!(
            modern.as_ref().err().is_some_and(is_algo_mismatch),
            "옛 서버라면 기본 목록으로는 협상이 실패해야 한다"
        );
        // 2) 폴백을 포함한 경로로는 붙고, 인증까지 통과해야 한다.
        let addr = &host;
        let (mut h, used_legacy) = connect_compat(&o, |cfg| {
            let hd = handler();
            async move { russh::client::connect(cfg, (addr.as_str(), port), hd).await }
        })
        .await
        .expect("레거시 폴백으로 연결");
        assert!(used_legacy, "레거시로 붙었다고 보고해야 한다");
        assert!(
            matches!(
                h.authenticate_password(&user, &pass).await,
                Ok(russh::client::AuthResult::Success)
            ),
            "옛 서버에서 비밀번호 인증까지 되어야 실제로 쓸 수 있다"
        );
    }

    #[test]
    fn keepalive_zero_disables() {
        let off = make_config(&ConnOpts { keepalive_secs: 0, ..Default::default() }, false);
        assert!(off.keepalive_interval.is_none());
        let on = make_config(&ConnOpts::default(), false);
        assert_eq!(on.keepalive_interval.map(|d| d.as_secs()), Some(30));
    }
}
