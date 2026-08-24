//! 대용량 이진 편집을 위한 **조각 표(piece table)** — 원본은 mmap, 편집은 조각으로.
//!
//! ## 왜 이렇게 하는가
//!
//! 예전 HEX 버퍼는 `Vec<u8>` 하나였다. 파일 전체를 RAM에 올리니 16MB(옛 `HEX_CAP`)를 넘으면
//! 아예 편집을 막고 읽기 전용 뷰어로 떨어뜨렸다(사용자 보고 2026-08-22: "편집을 눌러도
//! 뷰어로 열린다").
//!
//! MS-DOS 시절 EMM386은 64KB 창에 확장 메모리를 16KB씩 갈아 끼워 640KB 벽을 넘었다.
//! 64비트 Windows에는 그 벽이 없고, **커널이 같은 일을 더 잘한다** — 파일을 주소 공간에
//! 걸어 두면(mmap) 실제로 만진 페이지만 올라오고 안 쓰면 내려간다. 백킹 스토어가 스왑이
//! 아니라 파일 자체다.
//!
//! 그래서 빌려올 것은 **페이징**이고, 없던 나머지 절반이 이 조각 표다:
//!
//! ```text
//! 원본 ── mmap, 절대 고치지 않는다 (OS가 페이징)
//! 덧댐 ── 새로 넣은 바이트만 (작다)
//! 조각 ── [원본 0..1000] [덧댐 0..5] [원본 1000..끝]
//! ```
//!
//! 메모리가 **파일 크기가 아니라 편집 횟수에 비례**한다. 10GB 파일 한가운데 한 바이트를
//! 넣어도 조각 하나가 늘 뿐이다. 저장도 조각을 순서대로 흘려 쓰면 되므로 문서를 통째로
//! 메모리에 올릴 필요가 없다.

use std::io::{Result as IoResult, Write};
use std::path::Path;

/// 원본 바이트가 어디 있는가.
enum Orig {
    /// 파일을 주소 공간에 걸었다(대용량 경로).
    ///
    /// 주의: 매핑해 둔 파일을 다른 프로그램이 **줄이면** 그 페이지를 읽을 때 죽는다.
    /// 열어 둔 동안 파일이 바뀌지 않는다는 가정 위에서 동작한다(대용량 뷰어와 같은 전제).
    Map(memmap2::Mmap),
    /// 메모리에 이미 있는 바이트(작은 파일·클립보드·시험).
    Mem(Vec<u8>),
    /// 원본 없음(빈 문서).
    None,
}

impl Orig {
    fn slice(&self) -> &[u8] {
        match self {
            Orig::Map(m) => m,
            Orig::Mem(v) => v,
            Orig::None => &[],
        }
    }
}

/// 조각 하나 — 원본 또는 덧댐 버퍼의 한 구간.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Piece {
    /// true면 덧댐 버퍼, false면 원본.
    added: bool,
    off: usize,
    len: usize,
}

/// 조각 표로 표현한 바이트 열. 겉보기에는 하나의 긴 바이트 배열처럼 쓴다.
pub struct HexData {
    orig: Orig,
    add: Vec<u8>,
    pieces: Vec<Piece>,
    len: usize,
}

impl HexData {
    /// 메모리에 있는 바이트로 만든다(작은 파일·클립보드·시험).
    pub fn from_vec(v: Vec<u8>) -> Self {
        let len = v.len();
        let pieces = if len == 0 { Vec::new() } else { vec![Piece { added: false, off: 0, len }] };
        Self { orig: Orig::Mem(v), add: Vec::new(), pieces, len }
    }

    /// 파일을 매핑해서 연다. **파일 크기만큼 메모리를 쓰지 않는다.**
    pub fn map_file(path: &Path) -> IoResult<Self> {
        let f = std::fs::File::open(path)?;
        let len = f.metadata()?.len() as usize;
        if len == 0 {
            return Ok(Self { orig: Orig::None, add: Vec::new(), pieces: Vec::new(), len: 0 });
        }
        // SAFETY: 읽기 전용 매핑이다. 매핑해 둔 동안 다른 프로그램이 파일을 줄이면 접근이
        // 실패할 수 있다는 것이 이 API의 알려진 전제이며, 대용량 뷰어(editbig)도 같은 전제로
        // 동작한다. 우리는 매핑을 절대 수정하지 않고, 편집은 전부 덧댐 버퍼로 간다.
        let map = unsafe { memmap2::Mmap::map(&f)? };
        Ok(Self {
            orig: Orig::Map(map),
            add: Vec::new(),
            pieces: vec![Piece { added: false, off: 0, len }],
            len,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 조각 개수 — 편집이 얼마나 쌓였는지(시험·진단용).
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    fn src(&self, p: &Piece) -> &[u8] {
        let s = if p.added { &self.add[..] } else { self.orig.slice() };
        &s[p.off..p.off + p.len]
    }

    /// 한 바이트. 범위를 벗어나면 None.
    pub fn get(&self, at: usize) -> Option<u8> {
        if at >= self.len {
            return None;
        }
        let mut seen = 0usize;
        for p in &self.pieces {
            if at < seen + p.len {
                return Some(self.src(p)[at - seen]);
            }
            seen += p.len;
        }
        None
    }

    /// `[at, at+n)` 구간을 복사해 돌려준다(화면 한 페이지·검사기 등 **작은 구간**용).
    pub fn read(&self, at: usize, n: usize) -> Vec<u8> {
        let end = at.saturating_add(n).min(self.len);
        if at >= end {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(end - at);
        let mut seen = 0usize;
        for p in &self.pieces {
            let (ps, pe) = (seen, seen + p.len);
            seen = pe;
            if pe <= at {
                continue;
            }
            if ps >= end {
                break;
            }
            let lo = at.saturating_sub(ps);
            let hi = (end - ps).min(p.len);
            out.extend_from_slice(&self.src(p)[lo..hi]);
        }
        out
    }

    /// `[at, at+del)`을 지우고 그 자리에 `ins`를 넣는다(덮어쓰기·삽입·삭제 공용).
    ///
    /// 원본은 건드리지 않는다 — 조각을 쪼개고 다시 잇는 것이 전부다.
    pub fn splice(&mut self, at: usize, del: usize, ins: &[u8]) {
        let at = at.min(self.len);
        let del = del.min(self.len - at);
        let (start, end) = (at, at + del);

        // 앞·뒤를 **따로** 모은다. 여기서 바로 합치면 방금 만든 분할점이 도로 사라진다
        // (합치기 조건이 `앞의 끝 == 뒤의 시작`이라, 쪼갠 두 쪽이 정확히 그 모양이다).
        let (mut before, mut after) = (Vec::new(), Vec::new());
        let mut seen = 0usize;
        for p in &self.pieces {
            let (ps, pe) = (seen, seen + p.len);
            seen = pe;
            if ps < start {
                let keep = (start - ps).min(p.len);
                if keep > 0 {
                    before.push(Piece { added: p.added, off: p.off, len: keep });
                }
            }
            if pe > end {
                let skip = end.saturating_sub(ps);
                after.push(Piece { added: p.added, off: p.off + skip, len: p.len - skip });
            }
        }

        let mut next = before;
        if !ins.is_empty() {
            let off = self.add.len();
            self.add.extend_from_slice(ins);
            next.push(Piece { added: true, off, len: ins.len() });
        }
        next.extend(after);

        // 다 잇고 나서 한 번에 합친다. 새 조각이 사이에 끼어 있으면 양쪽은 합쳐지지 않고,
        // 순수 삭제였다면 이어지는 두 쪽만 알맞게 합쳐진다.
        let mut merged: Vec<Piece> = Vec::with_capacity(next.len());
        for p in next {
            push(&mut merged, p);
        }
        self.len = self.len - del + ins.len();
        self.pieces = merged;
    }

    /// 조각을 순서대로 훑으며 `(시작 오프셋, 바이트)`를 넘긴다 — **복사하지 않는다.**
    ///
    /// 줄 인덱스를 세우거나 전체를 훑어 검색할 때 쓴다. `read`로 같은 일을 하면 문서 크기만큼
    /// 복사본이 생기지만, 여기서는 원본 매핑을 그대로 빌려주므로 1GB든 10GB든 추가 메모리가 없다.
    pub fn scan_chunks(&self, mut f: impl FnMut(usize, &[u8])) {
        let mut seen = 0usize;
        for p in &self.pieces {
            f(seen, self.src(p));
            seen += p.len;
        }
    }

    /// 전체를 순서대로 흘려 쓴다(저장). 문서를 메모리에 모으지 않는다.
    pub fn write_to(&self, w: &mut impl Write) -> IoResult<()> {
        for p in &self.pieces {
            w.write_all(self.src(p))?;
        }
        Ok(())
    }

    /// 전부를 한 벌로 복사한다 — **작은 문서에서만** 쓴다(클립보드 등).
    pub fn to_vec(&self) -> Vec<u8> {
        self.read(0, self.len)
    }

    /// `from`부터 `needle`이 처음 나오는 위치. 구간을 겹쳐 가며 훑어 조각 경계를 넘긴다.
    pub fn find(&self, needle: &[u8], from: usize) -> Option<usize> {
        if needle.is_empty() || needle.len() > self.len {
            return None;
        }
        const CHUNK: usize = 1 << 16;
        let overlap = needle.len() - 1;
        let mut at = from.min(self.len);
        while at < self.len {
            let want = CHUNK + overlap;
            let buf = self.read(at, want);
            if buf.len() < needle.len() {
                return None;
            }
            if let Some(i) = buf.windows(needle.len()).position(|w| w == needle) {
                return Some(at + i);
            }
            at += buf.len() - overlap;
        }
        None
    }
}

/// 길이 0 조각은 버리고, 같은 원본에서 **바로 이어지는** 조각은 하나로 합친다.
/// 합치지 않으면 한 글자씩 이어 칠 때 조각이 무한정 늘어난다.
fn push(v: &mut Vec<Piece>, p: Piece) {
    if p.len == 0 {
        return;
    }
    if let Some(last) = v.last_mut() {
        if last.added == p.added && last.off + last.len == p.off {
            last.len += p.len;
            return;
        }
    }
    v.push(p);
}
