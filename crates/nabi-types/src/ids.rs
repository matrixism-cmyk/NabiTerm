//! 안정적 ID 뉴타입.
//!
//! 탭을 다른 OS 창으로 옮겨도 PaneId가 유지되어 오케스트레이터 라우팅이 안전하다.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

macro_rules! id_newtype {
    ($name:ident, $counter:ident, $next:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        pub struct $name(pub u64);

        impl $name {
            pub const fn new(v: u64) -> Self {
                Self(v)
            }
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}#{}", stringify!($name), self.0)
            }
        }

        static $counter: AtomicU64 = AtomicU64::new(1);

        /// 프로세스 전역 단조 증가 ID를 발급한다.
        pub fn $next() -> $name {
            $name($counter.fetch_add(1, Ordering::Relaxed))
        }
    };
}

id_newtype!(PaneId, PANE_COUNTER, next_pane_id);
id_newtype!(WindowId, WINDOW_COUNTER, next_window_id);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonic_and_unique() {
        let a = next_pane_id();
        let b = next_pane_id();
        assert!(b.get() > a.get());
        assert_ne!(a, b);
    }
}
