//! 파일 보기 모드(윈도우 탐색기식): 자세히/내용/목록/큰·작은 아이콘/타일. 로컬·SFTP 공용.

/// 보기 모드.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum ViewMode {
    #[default]
    Details,
    Content,
    List,
    LargeIcons,
    SmallIcons,
    Tile,
}

impl ViewMode {
    /// 메뉴 표시용 i18n 키.
    pub(crate) fn key(self) -> &'static str {
        match self {
            ViewMode::Details => "view.details",
            ViewMode::Content => "view.content",
            ViewMode::List => "view.list",
            ViewMode::LargeIcons => "view.large",
            ViewMode::SmallIcons => "view.small",
            ViewMode::Tile => "view.tile",
        }
    }
    /// 설정 영속용 정수 표현(기존 값 유지 + Content=5).
    pub(crate) fn to_u8(self) -> u8 {
        match self {
            ViewMode::Details => 0,
            ViewMode::List => 1,
            ViewMode::LargeIcons => 2,
            ViewMode::SmallIcons => 3,
            ViewMode::Tile => 4,
            ViewMode::Content => 5,
        }
    }
    pub(crate) fn from_u8(n: u8) -> ViewMode {
        match n {
            1 => ViewMode::List,
            2 => ViewMode::LargeIcons,
            3 => ViewMode::SmallIcons,
            4 => ViewMode::Tile,
            5 => ViewMode::Content,
            _ => ViewMode::Details,
        }
    }
    /// 선택 콤보용 전체 목록.
    pub(crate) fn all() -> [ViewMode; 6] {
        [
            ViewMode::Details,
            ViewMode::Content,
            ViewMode::List,
            ViewMode::LargeIcons,
            ViewMode::SmallIcons,
            ViewMode::Tile,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::ViewMode;

    #[test]
    fn view_u8_roundtrip() {
        for m in ViewMode::all() {
            assert_eq!(ViewMode::from_u8(m.to_u8()), m);
        }
        assert_eq!(ViewMode::from_u8(99), ViewMode::Details);
    }
}
