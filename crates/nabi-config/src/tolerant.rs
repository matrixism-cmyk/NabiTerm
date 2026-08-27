//! **어긋난 값 하나 때문에 설정을 통째로 잃지 않는다**(배치 AA).
//!
//! `load`는 원래 `extract().unwrap_or_default()`였다. 그래서 `config.toml`에 손으로
//! `font_size = "14"`라고 적으면 — 따옴표 하나 — 쌓아 둔 **모든 설정이 기본값으로** 돌아갔다.
//! 그런데 아무 말도 없으니 왜 초기화됐는지 알 수도 없다.
//!
//! ## 왜 구역별 폴백으로는 모자란가
//!
//! 처음에는 구역(`[appearance]`·`[terminal]`…)별로 다시 시도하게 고쳤다. 그러면 깨진 구역
//! **전체**를 잃는다 — 값 하나 때문에 그 구역의 나머지 서른 개도 함께 날아간다.
//! 그리고 **평평한 설정**(nabiPad의 `EditorConfig`)에는 구역이 없어 아예 적용되지 않는다.
//!
//! ## 그래서 어긋난 키만 버린다
//!
//! figment은 실패한 **키 경로**를 알려 준다(`Error::path`). 그 키만 빼고 다시 읽는다.
//! 여러 개가 어긋나 있으면 하나씩 빼며 반복한다. 남은 값은 전부 살아남는다.
//!
//! 되풀이 횟수에 상한을 둔다 — 오류가 키를 안 알려 주는 경우(경로가 빈 경우)에는 더 뺄 것이
//! 없으므로 그 자리에서 멈추고 기본값으로 간다. 상한이 없으면 그런 오류에서 영원히 돈다.

use figment::providers::{Env, Format, Serialized, Toml};
use figment::Figment;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// 뺄 수 있는 만큼 빼 가며 읽는다. 상한은 넉넉하되 유한하게.
const MAX_DROPS: usize = 24;

/// 기본값 → TOML 파일 → 환경변수 계층으로 읽되, **파싱에 걸리는 키만 버린다**.
///
/// 돌려주는 것은 `(설정, 버린 키들)`. 버린 키를 함께 주는 이유는 조용히 넘어가지 않기 위해서다 —
/// 부르는 쪽이 사용자에게 "이 줄은 못 읽어서 기본값을 씁니다"라고 말할 수 있어야 한다.
pub(crate) fn extract_tolerant<T>(file: &std::path::Path, env_prefix: &str) -> (T, Vec<String>)
where
    T: Default + Serialize + DeserializeOwned,
{
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let mut tree: toml::Value = match toml::from_str(&text) {
        Ok(v) => v,
        // 파일 자체가 TOML이 아니면 뺄 키를 고를 수 없다. 기본값으로 간다.
        Err(_) => return (T::default(), Vec::new()),
    };
    let mut dropped = Vec::new();
    for _ in 0..MAX_DROPS {
        let body = toml::to_string_pretty(&tree).unwrap_or_default();
        let fig = Figment::from(Serialized::defaults(T::default()))
            .merge(Toml::string(&body))
            .merge(Env::prefixed(env_prefix));
        match fig.extract::<T>() {
            Ok(v) => return (v, dropped),
            Err(e) => {
                let path = strip_profile(&e.path);
                // 경로를 모르면 더 해 볼 것이 없다.
                if path.is_empty() || !remove_at(&mut tree, &path) {
                    return (T::default(), dropped);
                }
                dropped.push(path.join("."));
            }
        }
    }
    (T::default(), dropped)
}

/// figment 경로 앞의 프로파일 이름(`default`)을 뗀다 — 파일 안에는 그 층이 없다.
fn strip_profile(path: &[String]) -> Vec<String> {
    match path.first().map(String::as_str) {
        Some("default" | "global") => path[1..].to_vec(),
        _ => path.to_vec(),
    }
}

/// 나무에서 그 경로의 값을 지운다. 지웠으면 `true`.
///
/// 배열 원소가 어긋난 경우(경로 끝이 숫자) **배열 전체를 지운다.** 한 칸만 빼면 남은 칸의
/// 자리가 밀려 사용자가 적은 것과 다른 뜻이 되는데, 그것이 조용히 일어나면 더 나쁘다.
fn remove_at(tree: &mut toml::Value, path: &[String]) -> bool {
    let Some((last, parents)) = path.split_last() else {
        return false;
    };
    let mut cur = tree;
    for p in parents {
        match cur {
            toml::Value::Table(t) => match t.get_mut(p) {
                Some(next) => cur = next,
                None => return false,
            },
            // 배열 안으로 들어가야 하면 그 배열을 통째로 지우는 편이 안전하다.
            _ => return false,
        }
    }
    match cur {
        toml::Value::Table(t) => t.remove(last).is_some(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    #[serde(default)]
    struct Flat {
        size: f32,
        name: String,
        on: bool,
    }
    impl Default for Flat {
        fn default() -> Self {
            Self { size: 14.0, name: "d".into(), on: false }
        }
    }

    fn write(text: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("nabi-tol-{}-{}", std::process::id(), text.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("c.toml");
        std::fs::write(&f, text).unwrap();
        (dir, f)
    }

    #[test]
    fn a_clean_file_loads_untouched() {
        let (d, f) = write("size = 20.0\nname = \"x\"\non = true\n");
        let (got, dropped): (Flat, _) = extract_tolerant(&f, "NABI_TOL_A_");
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(got, Flat { size: 20.0, name: "x".into(), on: true });
        assert!(dropped.is_empty());
    }

    /// **이 시험이 이 파일의 이유다.** 한 값이 어긋나도 나머지는 살아남는다.
    #[test]
    fn one_bad_value_only_loses_itself() {
        let (d, f) = write("size = \"열넷\"\nname = \"x\"\non = true\n");
        let (got, dropped): (Flat, _) = extract_tolerant(&f, "NABI_TOL_B_");
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(got.size, 14.0, "어긋난 값만 기본값이 된다");
        assert_eq!(got.name, "x", "옆 값은 살아남아야 한다");
        assert!(got.on, "옆 값은 살아남아야 한다");
        assert_eq!(dropped, vec!["size".to_string()], "무엇을 버렸는지 알려 준다");
    }

    #[test]
    fn several_bad_values_are_dropped_one_by_one() {
        let (d, f) = write("size = \"a\"\nname = 3\non = true\n");
        let (got, dropped): (Flat, _) = extract_tolerant(&f, "NABI_TOL_C_");
        let _ = std::fs::remove_dir_all(&d);
        assert!(got.on, "멀쩡한 값 하나는 끝까지 남는다");
        assert_eq!(dropped.len(), 2);
    }

    #[test]
    fn a_file_that_is_not_toml_falls_back_quietly() {
        let (d, f) = write("이건 TOML 이 아니다 {{{");
        let (got, dropped): (Flat, _) = extract_tolerant(&f, "NABI_TOL_D_");
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(got, Flat::default());
        assert!(dropped.is_empty(), "뺄 키를 고를 수 없으니 버린 것도 없다");
    }

    #[test]
    fn a_missing_file_is_just_defaults() {
        let (got, dropped): (Flat, _) =
            extract_tolerant(std::path::Path::new("nowhere-at-all.toml"), "NABI_TOL_E_");
        assert_eq!(got, Flat::default());
        assert!(dropped.is_empty());
    }

    #[test]
    fn profile_prefix_is_stripped() {
        assert_eq!(strip_profile(&["default".into(), "a".into()]), vec!["a".to_string()]);
        assert_eq!(strip_profile(&["a".into(), "b".into()]), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn remove_at_handles_nesting_and_absence() {
        let mut v: toml::Value = toml::from_str("[s]\nx = 1\n").unwrap();
        assert!(remove_at(&mut v, &["s".into(), "x".into()]));
        assert!(!remove_at(&mut v, &["s".into(), "x".into()]), "두 번째는 지울 것이 없다");
        assert!(!remove_at(&mut v, &[]), "빈 경로는 지울 수 없다");
    }
}
