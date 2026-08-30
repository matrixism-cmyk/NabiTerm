//! **이어받아도 되는가** — 남은 부분 파일이 지금의 원격 파일에서 나온 것인지 확인한다.
//!
//! ## 왜 필요한가
//!
//! 전송이 끊기면 `{local}.filepart` 가 남고, 다시 시도할 때 그 뒤부터 이어받는다.
//! 큐는 세션을 넘어 되살아나므로(`sftpqpersist`), **어제 받다 만 조각에 오늘 받은 바이트가
//! 이어 붙는 일**이 생길 수 있다. 그사이 원격 파일이 바뀌었다면 결과는 앞뒤가 다른 파일이다.
//!
//! 크기 검사로는 못 잡는다 — 파일이 바뀌어도 크기는 그대로일 수 있고, 커졌다면 이어받기가
//! 오히려 "성공"으로 끝난다. 해시 검증(`hashcheck`)은 잡지만 기본이 꺼져 있고, 다 받은
//! **뒤에야** 안다.
//!
//! ## 어떻게 아는가
//!
//! 부분 파일을 처음 만들 때 그 원격 파일의 **크기와 수정 시각을 옆에 적어 둔다**
//! (`{local}.filepart.src`). 이어받기 전에 지금 원격의 값과 맞춰 보고, 다르면 처음부터 받는다.
//!
//! 서버 시계와 우리 시계를 비교하지 않는다 — **같은 서버가 준 두 값**을 비교할 뿐이라
//! 시계가 틀어져 있어도 상관없다. 부분 파일 자신의 수정 시각을 쓰지 않는 까닭이 그것이다.

/// 부분 파일이 어느 원격 파일에서 나왔는지 — 옆에 적어 두는 값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub size: u64,
    pub mtime: u64,
}

impl Source {
    fn encode(&self) -> String {
        format!("{} {}", self.size, self.mtime)
    }

    fn decode(s: &str) -> Option<Self> {
        let (a, b) = s.trim().split_once(' ')?;
        Some(Self { size: a.parse().ok()?, mtime: b.parse().ok()? })
    }
}

/// 이어받아도 되는가 — 적어 둔 것과 지금 원격이 같아야 한다.
///
/// `recorded` 가 없으면(옛 판이 남긴 조각·적기 실패) **이어받지 않는다.** 모르면 다시 받는
/// 쪽이 맞다 — 다시 받는 값은 시간이고, 잘못 이어 붙인 값은 조용히 망가진 파일이다.
///
/// 지금 원격을 모를 때(`stat` 미지원 서버)도 이어받지 않는다. 같은 이유다.
pub fn may_resume(recorded: Option<Source>, now: Option<Source>) -> bool {
    match (recorded, now) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// 옆에 적어 두는 파일의 경로.
pub fn note_path(part: &str) -> String {
    format!("{part}.src")
}

/// 부분 파일이 어느 원격에서 나왔는지 적어 둔다.
pub fn write_note(part: &str, src: Source) {
    // 삼킴: 적기에 실패해도 전송은 계속되어야 한다. 적히지 않으면 다음번에 이어받지 않고
    // 처음부터 받을 뿐이라, 실패의 결과가 안전한 쪽이다.
    let _ = std::fs::write(note_path(part), src.encode());
}

/// 적어 둔 것을 읽는다. 없거나 깨졌으면 `None`.
pub fn read_note(part: &str) -> Option<Source> {
    Source::decode(&std::fs::read_to_string(note_path(part)).ok()?)
}

/// 다 받았으면 치운다 — 부분 파일과 함께 사라져야 한다.
pub fn clear_note(part: &str) {
    // 삼킴: 남아도 다음 전송이 덮어쓴다. 지우기 실패로 전송을 실패로 만들 이유가 없다.
    let _ = std::fs::remove_file(note_path(part));
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Source = Source { size: 100, mtime: 1_700_000_000 };

    #[test]
    fn 같은_파일이면_이어받는다() {
        assert!(may_resume(Some(A), Some(A)));
    }

    /// **이것이 이 모듈의 존재 이유다.** 크기가 같아도 시각이 다르면 다른 파일이다 —
    /// 크기 검사만으로는 못 잡는 자리다.
    #[test]
    fn 크기가_같아도_시각이_다르면_다시_받는다() {
        let changed = Source { mtime: A.mtime + 1, ..A };
        assert!(!may_resume(Some(A), Some(changed)));
    }

    #[test]
    fn 크기가_다르면_다시_받는다() {
        assert!(!may_resume(Some(A), Some(Source { size: 101, ..A })));
    }

    /// 모르면 다시 받는다 — 다시 받는 값은 시간이고, 잘못 이어 붙인 값은 망가진 파일이다.
    #[test]
    fn 모르면_이어받지_않는다() {
        assert!(!may_resume(None, Some(A)), "적어 둔 것이 없다(옛 판이 남긴 조각)");
        assert!(!may_resume(Some(A), None), "지금 원격을 모른다(stat 미지원 서버)");
        assert!(!may_resume(None, None));
    }

    #[test]
    fn 적은_것을_그대로_되읽는다() {
        assert_eq!(Source::decode(&A.encode()), Some(A));
        // 깨진 것은 없는 것으로 — 이어받지 않는 쪽으로 떨어진다.
        for bad in ["", "  ", "abc", "1", "1 x", "x 1", "1 2 3"] {
            let got = Source::decode(bad);
            assert!(got.is_none() || got == Some(Source { size: 1, mtime: 2 }), "{bad:?}");
        }
    }
}
