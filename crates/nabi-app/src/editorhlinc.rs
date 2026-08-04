//! 증분 구문 강조 — 바뀐 줄부터만 다시 계산하고, 파서 상태가 예전과 같아지면 멈춘다.
//!
//! 종전에는 전체 텍스트를 키로 한 프레임 캐시라 한 글자만 쳐도 문서 전체를 syntect로 다시
//! 돌렸다(수백 KB면 수십 ms — 타자가 끊긴다). 여기서는 줄 해시로 바뀐 구간을 찾고,
//! 100줄마다 저장해 둔 파서 체크포인트에서 재개해 **뒤쪽 상태가 예전과 일치하는 순간 중단**한다.
//! 그래서 한 글자 편집 비용이 문서 크기가 아니라 체크포인트 간격에 묶인다.
//!
//! 100줄 간격인 이유: syntect의 branch point가 128줄에서 만료되므로 그보다 짧아야 한다.

use crate::editorhlspans::{build_job, hl_line, LineSpans};
use crate::editorsyntax::Assets;
use egui::text::LayoutJob;
use std::cell::RefCell;
use std::collections::HashMap;
use syntect::highlighting::{HighlightState, Highlighter};
use syntect::parsing::{ParseState, ScopeStack};
use syntect::util::LinesWithEndings;

/// 파서 상태 체크포인트 간격(줄).
const CKPT: usize = 100;
/// 동시에 보관하는 문서 캐시 수(초과하면 통째로 비운다 — 탭을 많이 열어도 상한이 있다).
const MAX_DOCS: usize = 24;

/// 한 줄을 파싱하기 **직전**의 파서·강조 상태.
type State = (ParseState, HighlightState);
/// (줄 번호, 그 줄 직전 상태) 목록 — 줄 번호 오름차순.
type Ckpts = Vec<(usize, State)>;

/// 한 문서의 증분 강조 캐시.
pub(crate) struct IncHl {
    ext: String,
    theme: String,
    /// 줄별 해시(개행 포함) — 바뀐 구간 탐지용.
    hashes: Vec<u64>,
    spans: Vec<LineSpans>,
    ckpt: Ckpts,
    /// 마지막 결과와 그때의 글꼴 크기(무변경 프레임 재사용).
    job: Option<(u32, LayoutJob)>,
}

impl IncHl {
    pub(crate) fn new(ext: String, theme: String) -> Self {
        IncHl { ext, theme, hashes: Vec::new(), spans: Vec::new(), ckpt: Vec::new(), job: None }
    }

    fn reset(&mut self) {
        self.hashes.clear();
        self.spans.clear();
        self.ckpt.clear();
        self.job = None;
    }

    /// 바뀐 구간만 다시 강조한다. 반환값 = 조각이 갱신됐는지.
    fn refresh(&mut self, text: &str, a: &Assets) -> bool {
        let new_h: Vec<u64> = LinesWithEndings::from(text).map(line_hash).collect();
        if new_h == self.hashes && self.job.is_some() {
            return false;
        }
        let theme = a.themes.themes.get(&self.theme).or_else(|| a.themes.themes.values().next());
        let Some(th) = theme else {
            self.reset();
            return true;
        };
        let syntax = crate::editorsyntax::mapped_syntax(&self.ext)
            .and_then(|n| a.ps.find_syntax_by_name(&n))
            .or_else(|| a.ps.find_syntax_by_extension(&self.ext))
            .or_else(|| a.ps.find_syntax_by_first_line(text))
            .unwrap_or_else(|| a.ps.find_syntax_plain_text());
        let hl = Highlighter::new(th);
        let lines: Vec<&str> = LinesWithEndings::from(text).collect();
        let (p, s) = diff_range(&self.hashes, &new_h);
        let (old_n, new_n) = (self.hashes.len(), new_h.len());
        let delta = new_n as isize - old_n as isize;
        let new_suf = new_n - s;

        // 체크포인트를 앞(그대로 유효) / 뒤(위치만 이동 — 수렴 비교용)로 가른다. 사이는 버린다.
        let (mut head, mut tail): (Ckpts, Ckpts) = (Vec::new(), Vec::new());
        for (i, st) in std::mem::take(&mut self.ckpt) {
            if i <= p {
                head.push((i, st));
            } else if i >= old_n - s {
                tail.push(((i as isize + delta) as usize, st));
            }
        }
        let (k, mut cur) = match head.last() {
            Some((i, st)) => (*i, st.clone()),
            None => (0, (ParseState::new(syntax), HighlightState::new(&hl, ScopeStack::new()))),
        };

        let mut fresh: Vec<LineSpans> = Vec::new();
        let mut mid: Ckpts = Vec::new();
        let mut stop = new_n;
        for (i, line) in lines.iter().enumerate().skip(k) {
            // 접미부에 들어섰는데 그 자리의 옛 상태와 같아지면, 나머지 줄은 예전 결과 그대로다.
            if i >= new_suf && i > p {
                if let Ok(j) = tail.binary_search_by_key(&i, |(j, _)| *j) {
                    if tail[j].1 == cur {
                        stop = i;
                        break;
                    }
                }
            }
            if i > k && (i - k) % CKPT == 0 {
                mid.push((i, cur.clone()));
            }
            fresh.push(hl_line(line, &a.ps, &mut cur.0, &mut cur.1, &hl));
        }

        // 옛 조각의 대응 구간을 새 조각으로 갈아 끼운다(꼬리는 memmove만 — 깊은 복제 없음).
        let old_stop = (stop as isize - delta) as usize;
        let lo = k.min(self.spans.len());
        let hi = old_stop.clamp(lo, self.spans.len());
        self.spans.splice(lo..hi, fresh);
        head.append(&mut mid);
        head.extend(tail.into_iter().filter(|(j, _)| *j >= stop));
        self.ckpt = head;
        self.hashes = new_h;
        self.job = None;
        true
    }

    /// 이 문서의 LayoutJob(내용/글꼴이 그대로면 지난 결과 재사용).
    pub(crate) fn job(&mut self, text: &str, fsize: f32, a: &Assets) -> LayoutJob {
        let changed = self.refresh(text, a);
        let key = fsize.to_bits();
        if !changed {
            if let Some((k, j)) = &self.job {
                if *k == key {
                    return j.clone();
                }
            }
        }
        match build_job(text, &self.spans, fsize) {
            Some(j) => {
                self.job = Some((key, j.clone()));
                j
            }
            // 캐시 불변식이 깨졌다 — 평문으로 안전 복귀하고 다음 프레임에 전부 다시 계산한다.
            None => {
                self.reset();
                let font = egui::FontId::monospace(fsize);
                LayoutJob::simple(text.to_owned(), font, egui::Color32::GRAY, f32::INFINITY)
            }
        }
    }
}

/// 옛/새 줄 해시에서 (공통 접두 줄 수, 공통 접미 줄 수). 둘이 겹치지 않게 제한한다.
fn diff_range(old: &[u64], new: &[u64]) -> (usize, usize) {
    let n = old.len().min(new.len());
    let p = (0..n).find(|&i| old[i] != new[i]).unwrap_or(n);
    let mut s = 0;
    while s < n - p && old[old.len() - 1 - s] == new[new.len() - 1 - s] {
        s += 1;
    }
    (p, s)
}

fn line_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

thread_local! {
    /// 문서 id → 증분 캐시. UI는 단일 스레드라 thread_local로 충분하다.
    static STORE: RefCell<HashMap<u64, IncHl>> = RefCell::new(HashMap::new());
}

/// 문서(id)의 강조 LayoutJob. 확장자·테마가 바뀌면 그 문서 캐시만 새로 만든다.
pub(crate) fn job(id: u64, text: &str, ext: &str, fsize: f32) -> LayoutJob {
    let theme = crate::editorsyntax::current_theme();
    let lock = crate::editorsyntax::assets();
    let Ok(a) = lock.read() else { return LayoutJob::default() };
    STORE.with(|s| {
        let mut m = s.borrow_mut();
        if m.len() > MAX_DOCS {
            m.clear();
        }
        let e = m.entry(id).or_insert_with(|| IncHl::new(ext.to_string(), theme.clone()));
        if e.ext != ext || e.theme != theme {
            *e = IncHl::new(ext.to_string(), theme.clone());
        }
        e.job(text, fsize, &a)
    })
}

#[cfg(test)]
mod tests;
