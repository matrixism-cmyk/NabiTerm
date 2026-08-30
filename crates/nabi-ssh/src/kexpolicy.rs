//! **양자내성 연결 정책** — PQ 로 붙었는지 알려 주는 데서 그치지 않고, 요구할 수 있게 한다.
//!
//! ## 왜 필요한가
//!
//! 배지(`kexinfo`)는 협상이 끝난 뒤 "PQ 였다"를 보여 준다. 보는 것과 지키는 것은 다르다 —
//! 배지를 안 보면 그냥 지나간다. 지금 붙은 서버가 옛 키 교환만 지원한다는 사실을
//! **알아야 할 사람이 알아차리지 못한 채** 그대로 쓰게 된다.
//!
//! 오늘 오간 것을 지금 기록해 두었다가 양자 컴퓨터가 나온 뒤에 푸는 방식(harvest now,
//! decrypt later)이 PQ 를 서두르는 이유다. 그래서 "붙긴 붙었는데 옛 방식이었다"는
//! **말해 줘야 하는 사실**이다.
//!
//! ## 세 가지 태도
//!
//! * `auto`(기본) — 아무 말도 하지 않는다. 지금까지와 같다.
//! * `warn` — PQ 가 아니면 알린다. 붙는 것은 막지 않는다.
//! * `require` — PQ 가 아니면 **연결을 끊는다.**
//!
//! 막는 쪽을 기본으로 두지 않는 이유는 분명하다. 사내 장비·옛 OpenSSH 는 아직 PQ 를
//! 모른다. 기본이 막는 것이면 어제까지 되던 접속이 오늘 안 되고, 사용자는 우리가 고장
//! 났다고 판단한다. 지키고 싶은 사람이 켜는 것이 맞다.
//!
//! ## 판정은 순수 함수다
//!
//! 실제로 끊는 일은 부르는 쪽이 한다. 여기서는 **무엇을 해야 하는지만** 정한다 —
//! 그래야 서버 없이 시험할 수 있다.

/// 정책 — 설정 문자열과 짝이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KexPolicy {
    /// 아무 말도 하지 않는다.
    #[default]
    Auto,
    /// PQ 가 아니면 알린다(연결은 유지).
    Warn,
    /// PQ 가 아니면 끊는다.
    Require,
}

impl KexPolicy {
    /// 설정에 적힌 낱말을 정책으로. 모르는 낱말은 `Auto` — 설정 하나가 이상하다고
    /// 접속을 막으면, 고칠 방법이 접속뿐인 사람은 갇힌다.
    ///
    /// 이름이 `from_str` 이 아닌 까닭은 표준 트레이트와 헷갈리기 때문이다. 그쪽은 실패할
    /// 수 있어야 하는데(`Result`), 여기서는 무엇을 넣어도 답이 나온다.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "warn" => Self::Warn,
            "require" => Self::Require,
            _ => Self::Auto,
        }
    }

    /// 설정에 적을 낱말. `from_str` 과 왕복해야 한다.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Warn => "warn",
            Self::Require => "require",
        }
    }
}

/// 협상 결과를 보고 무엇을 할 것인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KexVerdict {
    /// 아무 일도 없다.
    Ok,
    /// 알리기만 한다.
    Warn,
    /// 연결을 끊는다.
    Reject,
}

/// 정책과 협상된 KEX 이름으로 판정한다.
///
/// `kex` 가 비어 있으면(아직 협상 전이거나 알 수 없음) 아무 판단도 하지 않는다 —
/// 모르는 것을 위반으로 다루면 협상 전에 끊긴다.
pub fn verdict(policy: KexPolicy, kex: &str) -> KexVerdict {
    if policy == KexPolicy::Auto || kex.trim().is_empty() || crate::kexinfo::is_pq_kex(kex) {
        return KexVerdict::Ok;
    }
    match policy {
        KexPolicy::Require => KexVerdict::Reject,
        _ => KexVerdict::Warn,
    }
}

/// 무엇을 알리고 끊을 것인가 — 문구 키와 "끊는가"를 함께 돌려준다. 없으면 아무 일도 없다.
///
/// 판단을 여기까지 끌어온 까닭은 **검증되는 넓이** 때문이다. 부르는 쪽에 갈래를 두면
/// 그 갈래는 실서버 없이 시험할 수 없다. 여기서 정하면 부르는 쪽은 두 줄로 줄고,
/// 어떤 연결에 무엇을 말할지는 전부 시험 아래로 들어온다.
pub fn notice(policy: KexPolicy, kex: &str) -> Option<(&'static str, bool)> {
    match verdict(policy, kex) {
        KexVerdict::Ok => None,
        KexVerdict::Warn => Some(("net.pq.notpq", false)),
        KexVerdict::Reject => Some(("net.pq.rejected", true)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 낱말과_정책이_왕복한다() {
        for p in [KexPolicy::Auto, KexPolicy::Warn, KexPolicy::Require] {
            assert_eq!(KexPolicy::parse(p.as_str()), p);
        }
        // 모르는 낱말은 아무것도 막지 않는 쪽으로 — 갇히지 않게.
        assert_eq!(KexPolicy::parse("zzz"), KexPolicy::Auto);
        assert_eq!(KexPolicy::parse(""), KexPolicy::Auto);
        // 대소문자·공백은 봐준다.
        assert_eq!(KexPolicy::parse("  REQUIRE "), KexPolicy::Require);
    }

    #[test]
    fn pq_면_어느_정책에서도_통과한다() {
        for p in [KexPolicy::Auto, KexPolicy::Warn, KexPolicy::Require] {
            assert_eq!(verdict(p, "mlkem768x25519-sha256"), KexVerdict::Ok);
            assert_eq!(verdict(p, "sntrup761x25519-sha512@openssh.com"), KexVerdict::Ok);
        }
    }

    #[test]
    fn pq_가_아니면_정책대로() {
        let old = "curve25519-sha256";
        assert_eq!(verdict(KexPolicy::Auto, old), KexVerdict::Ok);
        assert_eq!(verdict(KexPolicy::Warn, old), KexVerdict::Warn);
        assert_eq!(verdict(KexPolicy::Require, old), KexVerdict::Reject);
    }

    /// 모르는 것을 위반으로 다루면 협상이 끝나기 전에 끊긴다.
    #[test]
    fn 아직_모를_때는_판단하지_않는다() {
        for p in [KexPolicy::Auto, KexPolicy::Warn, KexPolicy::Require] {
            assert_eq!(verdict(p, ""), KexVerdict::Ok);
            assert_eq!(verdict(p, "   "), KexVerdict::Ok);
        }
    }
}

#[cfg(test)]
mod notice_tests {
    use super::*;

    /// 이 PC 의 OpenSSH 9.5 가 실제로 협상하는 이름이다(`ssh -Q kex` 로 확인, 2026-08-30).
    /// ML-KEM 을 제공하지 않으므로 정책을 켜면 반드시 걸린다.
    const REAL_NON_PQ: &str = "curve25519-sha256";

    #[test]
    fn 정책마다_무엇을_말할지() {
        assert_eq!(notice(KexPolicy::Auto, REAL_NON_PQ), None);
        assert_eq!(notice(KexPolicy::Warn, REAL_NON_PQ), Some(("net.pq.notpq", false)));
        assert_eq!(notice(KexPolicy::Require, REAL_NON_PQ), Some(("net.pq.rejected", true)));
    }

    /// PQ 로 붙었으면 어느 정책에서도 조용해야 한다 — 켜 놓고 매번 알림이 뜨면 곧 끈다.
    #[test]
    fn pq_면_아무_말도_하지_않는다() {
        for p in [KexPolicy::Auto, KexPolicy::Warn, KexPolicy::Require] {
            assert_eq!(notice(p, "mlkem768x25519-sha256"), None);
        }
    }

    /// **끊는 것은 require 뿐이다.** 이것이 어긋나면 알리기만 하려던 사람의 접속이 끊긴다.
    #[test]
    fn require_말고는_끊지_않는다() {
        for p in [KexPolicy::Auto, KexPolicy::Warn] {
            assert!(notice(p, REAL_NON_PQ).is_none_or(|(_, cut)| !cut));
        }
    }
}
