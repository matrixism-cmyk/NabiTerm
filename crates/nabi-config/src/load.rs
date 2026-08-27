//! figment 계층 병합 로드.

use crate::editor::EditorConfig;
use crate::paths::StorageLayout;
use crate::schema::AppConfig;
use figment::providers::{Env, Format, Serialized, Toml};
use figment::Figment;

/// 설정을 계층 병합으로 로드한다: 기본값 → config.toml → `NABI_` 환경변수.
///
/// ## 한 값이 어긋나도 나머지는 지킨다
///
/// 예전에는 통째로 `extract().unwrap_or_default()` 였다. 그래서 **값 하나의 타입이 어긋나면
/// 설정 전체가 기본값으로 돌아갔다** — 손으로 `font_size = "14"` 라고 적은 사람이 쌓아 둔
/// 모든 설정을 한 번에 잃는다. 그런데 아무 말도 없으니 왜 초기화됐는지 알 수도 없다.
///
/// 이제 통째로 실패하면 **구역별로 다시 시도한다.** 깨진 구역만 기본값이 되고 나머지는 산다.
/// 구역을 새로 더하면서 여기를 잊으면 `roundtrip_test` 의 구역별 시험이 잡는다.
pub fn load(layout: &StorageLayout) -> AppConfig {
    let fig = Figment::from(Serialized::defaults(AppConfig::default()))
        .merge(Toml::file(&layout.config_file))
        .merge(Env::prefixed("NABI_"));
    match fig.extract::<AppConfig>() {
        Ok(c) => c,
        Err(_) => AppConfig {
            appearance: fig.extract_inner("appearance").unwrap_or_default(),
            terminal: fig.extract_inner("terminal").unwrap_or_default(),
            telegram: fig.extract_inner("telegram").unwrap_or_default(),
        },
    }
}

/// nabiPad 설정을 로드한다: 기본값 → nabipad.toml → `NABI_EDITOR_` 환경변수.
/// 터미널 설정과 완전히 분리된 파일/네임스페이스를 쓴다(독립 프로그램화 대비).
pub fn load_editor(layout: &StorageLayout) -> EditorConfig {
    Figment::from(Serialized::defaults(EditorConfig::default()))
        .merge(Toml::file(&layout.editor_file))
        .merge(Env::prefixed("NABI_EDITOR_"))
        .extract()
        .unwrap_or_default()
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
