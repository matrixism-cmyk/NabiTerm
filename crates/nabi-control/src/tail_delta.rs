//! tail 델타 스트림 — 절대 줄 마커 기반으로 신규 줄만 추출(CP-5 G2).
//! 화면 전체 재덤프(중복·유실) 대신, 모델의 완료 줄 카운터 증가분만 읽는다.

use nabi_vt::TermModel;

/// 마커 증가분의 신규 줄을 돌려준다(없으면 None). last는 호출자가 보관.
/// 마커가 줄었으면(clear/RIS/alt 전환) 재동기화만 하고 방출하지 않는다.
pub fn delta(model: &TermModel, last: &mut usize) -> Option<String> {
    if model.alt_screen() {
        // 전체화면 앱(vim 등)은 줄 스트림 의미가 없음 — 마커만 추적.
        *last = model.line_marker();
        return None;
    }
    let cur = model.line_marker();
    match cur.cmp(last) {
        std::cmp::Ordering::Greater => {
            let lines = model.lines_abs_text(*last, cur);
            *last = cur;
            if lines.is_empty() {
                None
            } else {
                Some(lines.join("\n"))
            }
        }
        std::cmp::Ordering::Less => {
            *last = cur; // 화면 클리어 등 — 재동기화.
            None
        }
        std::cmp::Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_types::GridSize;

    #[test]
    fn streams_new_lines_without_dup_or_loss() {
        let mut m = TermModel::new(GridSize::new(20, 5), 100);
        let mut last = m.line_marker();
        m.process(b"one\r\ntwo\r\n");
        let d1 = delta(&m, &mut last).expect("신규 줄");
        assert_eq!(d1, "one\ntwo");
        // 같은 상태에서 재폴링 → 중복 없음.
        assert!(delta(&m, &mut last).is_none());
        m.process(b"three\r\n");
        assert_eq!(delta(&m, &mut last).unwrap(), "three");
    }

    #[test]
    fn scrollback_overflow_keeps_streaming() {
        let mut m = TermModel::new(GridSize::new(10, 3), 100);
        let mut last = m.line_marker();
        for i in 0..30 {
            m.process(format!("line{i}\r\n").as_bytes());
        }
        let all = delta(&m, &mut last).expect("줄들");
        assert!(all.starts_with("line0"));
        assert!(all.ends_with("line29"));
        assert_eq!(all.lines().count(), 30);
    }

    #[test]
    fn clear_resyncs_without_emitting_garbage() {
        let mut m = TermModel::new(GridSize::new(10, 3), 0); // 스크롤백 없음.
        let mut last = m.line_marker();
        m.process(b"a\r\nb\r\n");
        let _ = delta(&m, &mut last);
        m.process(b"\x1bc"); // RIS — 마커 감소.
        assert!(delta(&m, &mut last).is_none());
        m.process(b"c\r\n");
        assert_eq!(delta(&m, &mut last).unwrap(), "c");
    }
}
