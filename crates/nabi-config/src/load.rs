//! 설정 로드 — 계층 병합(기본값 → 파일 → 환경변수)에 **어긋난 키만 버리는** 관용을 얹는다.
//!
//! 병합과 관용은 `tolerant` 가 함께 맡는다. 여기는 어느 파일·어느 접두사인지만 정한다.

use crate::editor::EditorConfig;
use crate::paths::StorageLayout;
use crate::schema::AppConfig;

/// 설정을 계층 병합으로 로드한다: 기본값 → config.toml → `NABI_` 환경변수.
///
/// ## 어긋난 값 하나 때문에 전부 잃지 않는다
///
/// 예전에는 통째로 `extract().unwrap_or_default()` 였다. 그래서 **값 하나의 타입이 어긋나면
/// 설정 전체가 기본값으로 돌아갔다** — 손으로 `font_size = "14"` 라고 적은 사람이 쌓아 둔
/// 모든 설정을 한 번에 잃는다. 그런데 아무 말도 없으니 왜 초기화됐는지 알 수도 없다.
///
/// 이제 걸리는 **키만 버리고** 나머지는 그대로 읽는다(`tolerant`). 구역이 있든 없든 같은
/// 방식이라 nabiPad 설정도 같은 보호를 받는다.
pub fn load(layout: &StorageLayout) -> AppConfig {
    crate::tolerant::extract_tolerant(&layout.config_file, "NABI_").0
}

/// 읽으면서 **버린 키**까지 함께 돌려준다 — 사용자에게 알려 주려는 쪽이 쓴다.
///
/// 조용히 기본값으로 돌아가면 사용자는 자기 설정이 왜 사라졌는지 영영 모른다.
pub fn load_reporting(layout: &StorageLayout) -> (AppConfig, Vec<String>) {
    crate::tolerant::extract_tolerant(&layout.config_file, "NABI_")
}

/// nabiPad 설정을 로드한다: 기본값 → nabipad.toml → `NABI_EDITOR_` 환경변수.
/// 터미널 설정과 완전히 분리된 파일/네임스페이스를 쓴다(독립 프로그램화 대비).
pub fn load_editor(layout: &StorageLayout) -> EditorConfig {
    crate::tolerant::extract_tolerant(&layout.editor_file, "NABI_EDITOR_").0
}

#[cfg(test)]
mod tests {
    use super::load;
    use crate::paths::StorageLayout;
    use crate::persist::save;
    use crate::schema::{Appearance, AppConfig, DEFAULT_FONT_SIZE};

    #[test]
    fn from_base_builds_paths() {
        let l = StorageLayout::from_base(std::path::PathBuf::from("base"));
        assert!(l.config_file.ends_with("config.toml"));
        assert!(l.editor_file.ends_with("nabipad.toml"));
        assert!(l.sessions_file.ends_with("sessions.toml"));
    }

    #[test]
    fn missing_config_falls_back_to_default() {
        let l = StorageLayout::from_base(std::env::temp_dir().join("nabi-cfg-missing-xyz123"));
        assert_eq!(load(&l).appearance.font_size, DEFAULT_FONT_SIZE); // 없는 파일 → 기본값(앱이 죽지 않음).
    }

    #[test]
    fn save_then_load_roundtrips() {
        let base = std::env::temp_dir().join(format!("nabi-cfg-rt-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let l = StorageLayout::from_base(base.clone());
        let cfg = AppConfig { appearance: Appearance { font_size: 21.0, ..Default::default() }, ..Default::default() };
        save(&l.config_file, &cfg).unwrap();
        assert_eq!(load(&l).appearance.font_size, 21.0); // 저장 → 로드 왕복 보존.
        let _ = std::fs::remove_dir_all(&base);
    }
}
