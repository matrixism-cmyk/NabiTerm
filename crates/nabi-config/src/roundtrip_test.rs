//! **저장한 설정이 하나도 빠짐없이 돌아오는가**(배치 AA) — 필드를 손으로 열거하지 않는다.
//!
//! 기존 왕복 시험은 글꼴 크기 **한 필드**만 봤다. 그런데 이 저장소가 기록해 둔 위험은 그보다
//! 크다 — `load`가 `extract().unwrap_or_default()`라서 **한 필드의 파싱이 어긋나면 설정 전체가
//! 기본값으로 돌아간다.** 사용자가 쌓아 둔 모든 설정이 한 번에 사라지는 것이다.
//!
//! ## 왜 필드를 열거하지 않는가
//!
//! 손으로 적은 목록은 반드시 어긋난다(이 저장소는 설정 색인·단축키 표에서 이미 겪었다).
//! 새 필드를 더하고 시험을 잊으면 그 필드는 영영 검사되지 않는다.
//!
//! 그래서 **저장된 TOML 자체를 훑는다.** 기본 설정을 직렬화해 나온 나무의 잎을 전부 바꾸고,
//! 그것을 다시 읽어 저장한 뒤, 바꾼 값이 그대로 있는지 본다. 필드가 늘어나면 검사도 저절로
//! 늘어난다.

use crate::paths::StorageLayout;
use crate::schema::AppConfig;

/// TOML 나무의 잎을 전부 **기본값이 아닌 값**으로 바꾼다.
///
/// 기본값 그대로면 "사라졌는지"를 알 수 없다 — 잃어버린 필드도 기본값으로 돌아오기 때문에
/// 똑같아 보인다. 그래서 일부러 다르게 만든다.
fn mutate(v: &mut toml::Value) {
    match v {
        toml::Value::String(s) => {
            // 열거형처럼 정해진 값만 받는 문자열이 있을 수 있어 **뒤에 덧붙이기만** 한다.
            // 통째로 바꾸면 뜻이 달라져 읽는 쪽이 되돌릴 수도 있다.
            s.push_str("-rt");
        }
        toml::Value::Integer(i) => *i = i.wrapping_add(7),
        toml::Value::Float(f) => *f += 1.5,
        toml::Value::Boolean(b) => *b = !*b,
        toml::Value::Array(a) => {
            // 빈 배열은 **건드리지 않는다.** 원소 타입을 알 수 없어서 아무 값이나 넣으면
            // 타입이 어긋나고, 그러면 코드가 아니라 시험이 만든 실패가 된다.
            // (처음에 문자열을 넣었다가 `allowed_chats: Vec<i64>` 에서 걸렸다.)
            for x in a.iter_mut() {
                mutate(x);
            }
        }
        toml::Value::Table(t) => {
            for (_, x) in t.iter_mut() {
                mutate(x);
            }
        }
        toml::Value::Datetime(_) => {}
    }
}

/// 두 TOML 나무에서 **값이 달라진 자리**를 모은다(경로, 앞, 뒤).
fn diff(a: &toml::Value, b: &toml::Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (toml::Value::Table(x), toml::Value::Table(y)) => {
            for (k, xv) in x {
                match y.get(k) {
                    Some(yv) => diff(xv, yv, &format!("{path}.{k}"), out),
                    None => out.push(format!("{path}.{k} 가 사라졌다")),
                }
            }
        }
        (x, y) if x == y => {}
        (x, y) => out.push(format!("{path}: {x} → {y}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutate_changes_every_kind_of_leaf() {
        // 잎을 안 바꾸는 종류가 있으면 그 필드는 검사되지 않는다 — 먼저 이것부터 지킨다.
        let mut v: toml::Value = toml::from_str(
            "s = \"a\"\ni = 1\nf = 1.0\nb = true\nempty = []\n[t]\nn = 2\n",
        )
        .unwrap();
        let before = v.clone();
        mutate(&mut v);
        let mut d = Vec::new();
        diff(&before, &v, "", &mut d);
        // 빈 배열(`empty`)은 일부러 건드리지 않으므로 다섯 개만 바뀐다.
        assert_eq!(d.len(), 5, "바뀌지 않은 잎이 있다: {d:?}");
    }

    #[test]
    fn diff_reports_a_missing_key() {
        let a: toml::Value = toml::from_str("x = 1\ny = 2\n").unwrap();
        let b: toml::Value = toml::from_str("x = 1\n").unwrap();
        let mut d = Vec::new();
        diff(&a, &b, "", &mut d);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("사라졌다"), "{d:?}");
    }

    /// **이 시험이 이 파일의 핵심이다.** 바꾼 값이 저장·불러오기를 지나 그대로 돌아오는가.
    ///
    /// 하나라도 기본값으로 되돌아오면 그 필드는 디스크에서 못 읽히는 것이고, 그런 필드가
    /// 하나 있으면 **파싱이 통째로 실패해 모든 설정이 초기화될** 수도 있다.
    #[test]
    fn every_saved_field_comes_back() {
        let base = std::env::temp_dir().join(format!("nabi-rt-all-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let l = StorageLayout::from_base(base.clone());

        // 1) 기본 설정을 TOML 로 펼치고 잎을 전부 바꾼다.
        let text = toml::to_string_pretty(&AppConfig::default()).unwrap();
        let mut want: toml::Value = toml::from_str(&text).unwrap();
        mutate(&mut want);
        std::fs::write(&l.config_file, toml::to_string_pretty(&want).unwrap()).unwrap();

        // 2) 앱이 실제로 쓰는 길로 읽어 다시 펼친다.
        let got: toml::Value =
            toml::from_str(&toml::to_string_pretty(&crate::load::load(&l)).unwrap()).unwrap();

        let mut d = Vec::new();
        diff(&want, &got, "", &mut d);
        let _ = std::fs::remove_dir_all(&base);
        assert!(d.is_empty(), "저장한 값이 돌아오지 않았다:\n  {}", d.join("\n  "));
    }


    /// **한 구역이 깨져도 나머지는 살아남는가.**
    ///
    /// 구역 목록을 손으로 적지 않는다 — 기본 설정을 펼쳐 나온 최상위 키가 곧 구역이다.
    /// 그래서 구역을 새로 더하면서 `load` 의 폴백에 넣는 것을 잊으면 여기서 걸린다.
    #[test]
    fn a_broken_section_does_not_wipe_the_others() {
        let text = toml::to_string_pretty(&AppConfig::default()).unwrap();
        let base_tree: toml::Value = toml::from_str(&text).unwrap();
        let sections: Vec<String> = match &base_tree {
            toml::Value::Table(t) => t.keys().cloned().collect(),
            _ => panic!("설정이 표가 아니다"),
        };
        assert!(sections.len() >= 3, "구역이 너무 적다: {sections:?}");

        for broken in &sections {
            let mut tree = base_tree.clone();
            mutate(&mut tree); // 모든 값을 기본값이 아닌 것으로.
            // 이 구역에만 **타입이 어긋난 값**을 심는다. 사용자가 손으로 잘못 적은 상황이다.
            if let toml::Value::Table(t) = &mut tree {
                if let Some(toml::Value::Table(sec)) = t.get_mut(broken) {
                    sec.insert("nabi_rt_bad".into(), toml::Value::String("x".into()));
                    // 실제로 있는 필드의 타입을 어긋뜨려야 파싱이 깨진다.
                    let first_num = sec
                        .iter()
                        .find(|(_, v)| v.is_integer() || v.is_float() || v.is_bool())
                        .map(|(k, _)| k.clone());
                    if let Some(k) = first_num {
                        sec.insert(k, toml::Value::String("완전히 틀린 값".into()));
                    }
                }
            }
            let dir = std::env::temp_dir()
                .join(format!("nabi-rt-sec-{}-{broken}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let l = StorageLayout::from_base(dir.clone());
            std::fs::write(&l.config_file, toml::to_string_pretty(&tree).unwrap()).unwrap();
            let got: toml::Value =
                toml::from_str(&toml::to_string_pretty(&crate::load::load(&l)).unwrap()).unwrap();
            let _ = std::fs::remove_dir_all(&dir);

            // 깨뜨리지 않은 구역들은 바꾼 값을 그대로 들고 있어야 한다.
            for other in sections.iter().filter(|s| *s != broken) {
                let want = base_tree.get(other).cloned().unwrap();
                let mut want_m = want.clone();
                mutate(&mut want_m);
                let mut d = Vec::new();
                diff(&want_m, got.get(other).unwrap(), other, &mut d);
                assert!(
                    d.is_empty(),
                    "[{broken}] 이 깨졌다고 [{other}] 까지 잃었다:
  {}",
                    d.join("
  ")
                );
            }
        }
    }
}
