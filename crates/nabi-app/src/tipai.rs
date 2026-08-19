//! 사전에 없는 영문 팁의 **선택적** AI 번역(기본 꺼짐) — `claude -p` 헤드리스 호출 + 디스크 캐시.
//!
//! 왜 기본 꺼짐인가: 호출마다 사용자의 토큰·요금이 들고, 화면 내용이 외부로 나가며,
//! 폐쇄망에서는 동작하지 않는다. 켠 경우에도 **같은 문장은 한 번만** 번역하고 결과를
//! 디스크에 남겨(팁은 반복된다) 실제 호출 수를 최소화한다. 동시 요청은 1건으로 제한한다.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 번역 캐시 + 진행 중 요청 1건.
#[derive(Default)]
pub(crate) struct TipAi {
    cache: HashMap<String, String>,
    path: PathBuf,
    /// (원문, 결과 슬롯) — 백그라운드 스레드가 채운다.
    inflight: Option<(String, Arc<Mutex<Option<String>>>)>,
    /// 실패한 원문(빈 응답·CLI 없음) — 반복 호출로 비용을 낭비하지 않는다.
    failed: HashSet<String>,
}

impl TipAi {
    /// 캐시 파일을 읽어 초기화한다. `custom`이 비어 있지 않으면 그 경로를 쓴다 —
    /// 네트워크 드라이브·UNC(`\\server\share\tipcache.json`)를 지정하면 여러 PC의
    /// 번역이 **한 파일에 계속 누적**된다(사용자 요청 2026-08-19: 개발 서버에 모으기).
    pub(crate) fn load(dir: &Path, custom: &str) -> Self {
        let path = if custom.trim().is_empty() {
            dir.join("tipcache.json")
        } else {
            PathBuf::from(custom.trim())
        };
        let cache = read_map(&path);
        Self { cache, path, ..Default::default() }
    }

    /// 경로가 바뀌면 새 파일에서 다시 읽는다(설정 변경 즉시 반영).
    pub(crate) fn retarget(&mut self, dir: &Path, custom: &str) {
        let path = if custom.trim().is_empty() { dir.join("tipcache.json") } else { PathBuf::from(custom.trim()) };
        if path != self.path {
            self.cache = read_map(&path);
            self.path = path;
        }
    }

    /// 캐시된 번역(있으면).
    pub(crate) fn get(&self, en: &str) -> Option<&str> {
        self.cache.get(en).map(String::as_str)
    }

    /// 아직 번역이 없으면 백그라운드 번역을 시작한다(동시 1건, 실패한 문장은 건너뜀).
    pub(crate) fn request(&mut self, en: &str) {
        if self.inflight.is_some() || self.cache.contains_key(en) || self.failed.contains(en) {
            return;
        }
        let slot: Arc<Mutex<Option<String>>> = Arc::default();
        let (out, text) = (slot.clone(), en.to_owned());
        std::thread::spawn(move || {
            // 프롬프트는 ASCII 영어(주입 규칙과 동일한 원칙) — 원문도 영문 팁이다.
            let prompt = format!(
                "Translate this terminal tip into natural Korean. Output only the translation, one line.\n\n{text}"
            );
            let res = std::process::Command::new("claude")
                .args(["-p", &prompt])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|s| !s.is_empty() && s.chars().count() < 400);
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(res.unwrap_or_default());
        });
        self.inflight = Some((en.to_owned(), slot));
    }

    /// 완료된 번역을 수거해 캐시에 넣는다(변경되면 true). 매 프레임 호출해도 싸다.
    pub(crate) fn poll(&mut self) -> bool {
        let Some((en, slot)) = self.inflight.as_ref() else { return false };
        let done = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let Some(text) = done else { return false };
        let en = en.clone();
        self.inflight = None;
        if text.is_empty() {
            self.failed.insert(en); // CLI 없음·오류 → 다시 시도하지 않는다.
            return false;
        }
        self.cache.insert(en, text);
        self.save();
        true
    }

    /// 파일에 저장한다 — **먼저 다시 읽어 병합**한다. 같은 파일을 다른 PC·다른 세션이
    /// 함께 쓸 때(공유 폴더에 모으는 구성) 서로의 항목을 지우지 않기 위해서다.
    fn save(&mut self) {
        let mut merged = read_map(&self.path);
        merged.extend(self.cache.iter().map(|(k, v)| (k.clone(), v.clone())));
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(&merged) {
            if std::fs::write(&self.path, s).is_ok() {
                self.cache = merged; // 저장에 성공했을 때만 남의 항목까지 흡수한다.
            }
        }
    }
}

/// 캐시 파일을 읽는다(없거나 깨졌으면 빈 맵 — 번역 캐시는 잃어도 기능이 죽지 않는다).
fn read_map(path: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
