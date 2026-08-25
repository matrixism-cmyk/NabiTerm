//! 원격 여유 공간 — **올리기 전에** 들어갈 자리가 있는지 안다.
//!
//! `free_space()`는 진작 만들어 놓고 한 번도 부르지 않았다. 만들어 두고 안 쓰는 것은
//! 없는 것과 같다. 여기서 화면에 올리고, 올리기 전 경고에도 쓴다.
//!
//! 서버가 `statvfs`를 지원하지 않으면 알 수 없다. 그때는 **모른다고 말한다** — 0으로
//! 보여 주면 "가득 찼다"는 거짓말이 된다.

/// 올리려는 크기가 남은 자리보다 큰가. 여유를 모르면(None) 막지 않는다.
///
/// 여유가 딱 맞아도 실패하는 일이 잦다(파일시스템 예약분·블록 단위). 그래서 약간의
/// 여지를 두고 판단한다.
pub(crate) fn will_not_fit(need: u64, free: Option<u64>) -> bool {
    const MARGIN: u64 = 8 * 1024 * 1024; // 8MB.
    match free {
        Some(f) => need.saturating_add(MARGIN) > f,
        None => false,
    }
}

/// 상태 표시줄에 적을 글. 모르면 None(자리를 비운다 — 0으로 적으면 거짓말이다).
pub(crate) fn label(free: Option<u64>) -> Option<String> {
    free.map(|f| format!("\u{1f4be} {}", nabi_editor::humanfmt::human(f)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_fits_is_allowed() {
        assert!(!will_not_fit(100, Some(1_000_000_000)));
    }

    #[test]
    fn a_file_that_does_not_fit_is_flagged() {
        assert!(will_not_fit(2_000_000_000, Some(1_000_000_000)));
    }

    /// **여유가 딱 맞아도 막는다** — 예약분·블록 단위 때문에 실제로는 실패한다.
    #[test]
    fn an_exact_fit_is_still_flagged() {
        assert!(will_not_fit(1_000, Some(1_000)), "여지 없이 통과시켰다");
    }

    /// **모르면 막지 않는다** — statvfs를 지원하지 않는 서버에서 업로드를 못 하게 되면 안 된다.
    #[test]
    fn unknown_free_space_never_blocks() {
        assert!(!will_not_fit(u64::MAX, None));
    }

    /// 모르는 것을 0으로 보여 주면 "가득 찼다"는 거짓말이 된다.
    #[test]
    fn unknown_free_space_shows_nothing() {
        assert_eq!(label(None), None);
        assert!(label(Some(1024)).is_some());
    }

    #[test]
    fn the_label_is_human_readable() {
        let l = label(Some(5 * 1024 * 1024 * 1024)).unwrap();
        assert!(l.contains("5"), "{l}");
    }
}
