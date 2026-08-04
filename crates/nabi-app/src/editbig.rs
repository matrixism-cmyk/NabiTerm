//! 대용량 파일 가상화 뷰어(읽기 전용) — memmap2 + 백그라운드 라인 인덱스.
//! 보이는 줄만 디코드·렌더해 수백 MB도 즉시 열람. 편집은 후순위(E6).

use memmap2::Mmap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// 이 크기 이상이면 편집기 대신 뷰어로 연다.
pub(crate) const BIG_THRESHOLD: u64 = 2_000_000;

/// 메모리 매핑 파일 + 줄 시작 오프셋 인덱스(백그라운드 구축).
pub(crate) struct BigFile {
    map: Arc<Mmap>,
    /// 각 줄 시작 바이트 오프셋([0, l1, l2, …, len]). 인덱싱 중 점진적으로 증가.
    starts: Arc<RwLock<Vec<usize>>>,
    done: Arc<AtomicBool>,
    enc: &'static encoding_rs::Encoding,
    pub bytes: usize,
}

impl BigFile {
    /// 파일을 매핑하고 백그라운드 줄 인덱싱을 시작한다(실패 시 None).
    pub(crate) fn open(path: &Path) -> Option<BigFile> {
        let file = std::fs::File::open(path).ok()?;
        let map = Arc::new(unsafe { Mmap::map(&file).ok()? });
        let bytes = map.len();
        let enc = crate::editload::detect_encoding(&map[..bytes.min(64 * 1024)]);
        let starts = Arc::new(RwLock::new(vec![0usize]));
        let done = Arc::new(AtomicBool::new(false));
        let (m2, s2, d2) = (map.clone(), starts.clone(), done.clone());
        std::thread::spawn(move || {
            let mut local: Vec<usize> = Vec::with_capacity(65536);
            for (i, b) in m2.iter().enumerate() {
                if *b == b'\n' {
                    local.push(i + 1);
                    if local.len() >= 65536 {
                        if let Ok(mut g) = s2.write() {
                            g.extend_from_slice(&local);
                        }
                        local.clear();
                    }
                }
            }
            if let Ok(mut g) = s2.write() {
                g.extend_from_slice(&local);
                g.push(m2.len()); // 마지막 줄 끝.
            }
            d2.store(true, Ordering::Relaxed);
        });
        Some(BigFile { map, starts, done, enc, bytes })
    }

    /// 현재까지 인덱싱된 줄 수.
    pub(crate) fn line_count(&self) -> usize {
        self.starts.read().map(|s| s.len().saturating_sub(1).max(1)).unwrap_or(1)
    }

    pub(crate) fn ready(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }

    pub(crate) fn encoding(&self) -> &'static str {
        self.enc.name()
    }

    /// i번째 줄을 디코드한다(개행 제거). 인덱스 밖이면 빈 문자열.
    pub(crate) fn line(&self, i: usize) -> String {
        let Ok(starts) = self.starts.read() else {
            return String::new();
        };
        if i + 1 >= starts.len() {
            return String::new();
        }
        let (a, b) = (starts[i], starts[i + 1].min(self.map.len()));
        if a >= b {
            return String::new();
        }
        let (txt, _, _) = self.enc.decode(&self.map[a..b]);
        txt.trim_end_matches(['\n', '\r']).to_string()
    }
}
