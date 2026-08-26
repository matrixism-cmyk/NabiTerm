//! 브라우저의 **묶기·풀기** 동작 — 계산과 파일 쓰기는 `zipops`가 한다.
//!
//! 여기서 정하는 것은 **어디에 무엇을 만드나** 하나뿐이다.
//!
//! * 묶기: 고른 것이 여럿이면 폴더 이름을, 하나면 그 이름을 딴 `이름.zip`. 이미 있으면
//!   `(1)`을 붙인다 — 있는 파일을 말없이 덮어쓰지 않는다.
//! * 풀기: `이름.zip` → `이름/` 폴더. **현재 폴더에 그냥 쏟지 않는다** — 파일 수십 개가
//!   섞이면 무엇이 원래 있던 것인지 알 수 없게 된다.
//!
//! 목록 갱신은 따로 하지 않는다 — 브라우저가 폴더를 다시 읽는다(복제와 같은 길).

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::time::Instant;

impl NabiApp {
    /// 고른 항목들(없으면 `name` 하나)을 zip으로 묶는다.
    pub(crate) fn zip_selected(&mut self, name: String) {
        let dir = self.browser.path.clone();
        let mut names: Vec<String> = self.browser.multi.iter().cloned().collect();
        if names.is_empty() {
            names.push(name);
        }
        names.sort();
        // 이름: 하나면 그 이름, 여럿이면 이 폴더 이름을 딴다.
        let base = match names.len() {
            1 => std::path::Path::new(&names[0])
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| names[0].clone()),
            _ => dir
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "archive".to_string()),
        };
        let dest = crate::sftppath::dedup_name(|n| dir.join(n).exists(), &base, ".zip");
        let path = dir.join(&dest);
        match crate::zipops::create(&dir, &names, &path) {
            Ok(rep) => {
                let mut msg = format!("{} \u{2192} {dest} ({})", tr(self.lang, "browser.zipmake"), rep.done);
                if rep.truncated {
                    msg.push_str(&format!(" \u{b7} {}", tr(self.lang, "browser.ziptrunc")));
                }
                self.notify = Some((msg, Instant::now()));
            }
            Err(e) => {
                // 반쯤 쓰다 만 zip을 남기지 않는다 — 있으면 성공한 묶음으로 오해한다.
                let _ = std::fs::remove_file(&path);
                self.notify = Some((e, Instant::now()));
            }
        }
    }

    /// zip을 같은 이름의 새 폴더에 푼다.
    pub(crate) fn zip_extract(&mut self, name: String) {
        let dir = self.browser.path.clone();
        let src = dir.join(&name);
        let stem = std::path::Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        let folder = crate::sftppath::dedup_name(|n| dir.join(n).exists(), &stem, "");
        let dest = dir.join(&folder);
        match crate::zipops::extract(&src, &dest) {
            Ok(rep) => {
                let mut msg = format!("{} \u{2192} {folder} ({})", tr(self.lang, "browser.zipextract"), rep.done);
                // 건너뛴 것을 조용히 넘기지 않는다 — 푼 줄 알았는데 없는 파일이 생긴다.
                if rep.unsafe_paths > 0 {
                    msg.push_str(&format!(" \u{b7} {} {}", tr(self.lang, "browser.zipunsafe"), rep.unsafe_paths));
                }
                if rep.truncated {
                    msg.push_str(&format!(" \u{b7} {}", tr(self.lang, "browser.ziptrunc")));
                }
                self.notify = Some((msg, Instant::now()));
            }
            Err(e) => self.notify = Some((e, Instant::now())),
        }
    }

    /// 브라우저 액션에서 온 두 갈래를 한 번에 처리한다(호출측을 한 줄로 유지).
    pub(crate) fn apply_zip_acts(&mut self, make: Option<String>, extract: Option<String>) {
        if let Some(n) = make {
            self.zip_selected(n);
        }
        if let Some(n) = extract {
            self.zip_extract(n);
        }
    }
}
