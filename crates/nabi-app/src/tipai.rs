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
    /// 설정 폴더의 캐시 파일을 읽어 초기화한다(없으면 빈 캐시).
    pub(crate) fn load(dir: &Path) -> Self {
        let path = dir.join("tipcache.json");
        let cache = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { cache, path, ..Default::default() }
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
        if let Ok(s) = serde_json::to_string_pretty(&self.cache) {
            let _ = std::fs::write(&self.path, s);
        }
        true
    }
}
