//! 읽기 전용 뷰어에서 **한 구간만 꺼내 편집**한다(순수 계산은 `nabi_editor::textrange`).
//!
//! 왜 이것이 필요한지, 왜 되돌려 쓰지 않는지는 그쪽 문서에 있다. 여기서는 파일을 읽고
//! 문서를 여는 일만 한다.

use crate::app::NabiApp;
use nabi_editor::textrange;
use nabi_i18n::tr;
use nabi_types::PaneId;
use std::io::{Read, Seek, SeekFrom};

impl NabiApp {
    /// 뷰어의 지금 보이는 줄부터 한 구간을 꺼내 새 문서로 연다.
    pub(crate) fn open_range_from_viewer(&mut self, pane: PaneId, top_line: usize) {
        let Some(doc) = self.editors.get(&pane) else { return };
        let Some(big) = doc.big.as_ref() else { return };
        let (path, total) = (doc.path.clone(), big.bytes as u64);
        // 줄 번호로는 파일을 못 자른다 — 그 줄이 몇 번째 바이트인지 물어본다.
        // 아직 색인 중이면 그 줄을 모를 수 있다. 그때는 처음부터 꺼낸다(빈손보다 낫다).
        let start = big.line_start(top_line).unwrap_or(0);
        let (start, len) = textrange::plan(total, start, textrange::DEFAULT_SLICE);
        if len == 0 {
            self.notify = Some((tr(self.lang, "editor.range.empty").to_string(), std::time::Instant::now()));
            return;
        }
        let raw = match read_at(&path, start, len) {
            Ok(b) => b,
            Err(e) => {
                // 조용히 실패하지 않는다 — 눌렀는데 아무 일도 없으면 고장으로 보인다.
                self.notify = Some((format!("{}: {e}", tr(self.lang, "editor.range.failed")), std::time::Instant::now()));
                return;
            }
        };
        // 반쪽 줄을 버린다(가운데를 그냥 자르면 첫 줄과 끝 줄이 반쪽이 된다).
        let (a, b) = textrange::line_aligned(&raw, start == 0, start + len >= total);
        let slice = &raw[a..b];
        // 인코딩은 **원본에서 이미 알아낸 것**을 쓴다. 토막만 보고 다시 알아내면
        // 같은 파일에서 꺼낸 두 구간이 서로 다른 인코딩으로 열릴 수 있다.
        let enc = encoding_rs::Encoding::for_label(big.encoding().as_bytes()).unwrap_or(encoding_rs::UTF_8);
        let (text, _, _) = enc.decode(slice);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let title = textrange::slice_title(&name, start + a as u64, (b - a) as u64);
        // **경로를 비워서 연다.** 경로를 넣으면 Ctrl+S 가 원본 거대 파일을 이 토막으로
        // 덮어써 버린다 — 100GB 로그가 4MB 가 되는 사고다. "다른 이름으로 저장"만 되게 한다.
        let doc = crate::editor::EditorDoc::make(
            title,
            std::path::PathBuf::new(),
            None,
            text.into_owned(),
            true,
            self.font_size,
            big.encoding().to_string(),
            doc.eol,
        );
        self.add_editor_tab(doc);
    }
}

/// 파일의 그 자리에서 그만큼 읽는다. **파일 전체를 읽지 않는다** — 그게 이 기능의 전부다.
fn read_at(path: &std::path::Path, at: u64, len: u64) -> std::io::Result<Vec<u8>> {
    let mut f = std::fs::File::open(path)?;
    f.seek(SeekFrom::Start(at))?;
    let mut buf = vec![0u8; len as usize];
    // 끝에 닿아 덜 읽히는 경우가 있다(파일이 그 사이에 줄었거나 계산이 아슬아슬할 때).
    // 그때는 읽은 만큼만 쓴다 — 0 으로 채운 꼬리를 글로 보여 주면 안 된다.
    let mut got = 0usize;
    while got < buf.len() {
        match f.read(&mut buf[got..])? {
            0 => break,
            n => got += n,
        }
    }
    buf.truncate(got);
    Ok(buf)
}
