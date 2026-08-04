//! 인라인 이미지 — 바이트 스트림에서 Sixel(DCS)·iTerm(OSC 1337)·Kitty(APC G)를 관찰해
//! 디코드·배치한다. alacritty는 이들 시퀀스를 무시(no-op)하므로 병렬 관찰만으로 안전.
//!
//! 이미지 완성 시 커서 절대줄에 앵커를 기록하고 높이만큼 줄을 확보(LF)해 다음 출력이
//! 이미지를 덮지 않게 한다. 렌더러는 visible_images로 텍스처를 그린다.

use crate::grid::TermModel;
use nabi_image::DecodedImage;
use std::sync::Arc;

const MAX_BUF: usize = 16_000_000;

/// 화면에 배치된 인라인 이미지(절대 줄 기준 앵커).
pub struct PlacedImage {
    pub id: u64,
    pub anchor_abs: i64,
    pub rows: u16,
    pub img: Arc<DecodedImage>,
}

/// 이스케이프 문자열 시퀀스(DCS/OSC/APC) 스캐너. state: 0 평시/1 ESC/2 DCS/3 DCS-ESC/
/// 4 OSC/5 OSC-ESC/6 APC/7 APC-ESC.
#[derive(Default)]
pub(crate) struct EscScan {
    state: u8,
    kind: u8, // 2=DCS(sixel) 4=OSC(iterm) 6=APC(kitty)
    buf: Vec<u8>,
    osc_skip: bool, // OSC가 1337;File= 아니면 버퍼링 생략.
}

/// Kitty 청크 전송 누적(여러 APC에 걸친 base64 페이로드).
pub(crate) struct KittyPending {
    fmt: u32,
    w: u32,
    h: u32,
    b64: Vec<u8>,
    display: bool,
}

impl TermModel {
    /// 렌더러가 셀 픽셀 높이를 알려준다(이미지 높이→줄 수 변환에 사용).
    pub fn set_cell_px(&mut self, h: f32) {
        if h >= 1.0 {
            self.cell_px_h = h;
        }
    }

    /// process()에서 바이트마다 호출 — 이미지 시퀀스를 관찰하고 종료 시 디코드·배치한다.
    pub(crate) fn esc_observe(&mut self, b: u8) {
        match self.escan.state {
            0 => {
                if b == 0x1B {
                    self.escan.state = 1;
                }
            }
            1 => self.esc_intro(b),
            2 => self.collect(b, 3),         // DCS 본문.
            3 => self.collect_st(b, 2),      // DCS ST 대기.
            4 => self.osc_body(b),           // OSC 본문(BEL/ST 종료).
            5 => self.collect_st(b, 4),      // OSC ST 대기.
            6 => self.collect(b, 7),         // APC 본문.
            _ => self.collect_st(b, 6),      // APC ST 대기.
        }
    }

    /// ESC 다음 한 글자로 시퀀스 종류를 정한다(P=DCS, ]=OSC, _=APC, 그 외=종료).
    fn esc_intro(&mut self, b: u8) {
        self.escan.buf.clear();
        self.escan.osc_skip = false;
        match b {
            b'P' => {
                self.escan.kind = 2;
                self.escan.state = 2;
            }
            b']' => {
                self.escan.kind = 4;
                self.escan.state = 4;
            }
            b'_' => {
                self.escan.kind = 6;
                self.escan.state = 6;
            }
            _ => self.escan.state = 0,
        }
    }

    /// 본문 1바이트: ESC면 ST 대기 상태로, 아니면 버퍼에 누적.
    fn collect(&mut self, b: u8, esc_state: u8) {
        if b == 0x1B {
            self.escan.state = esc_state;
        } else if self.escan.buf.len() < MAX_BUF {
            self.escan.buf.push(b);
        }
    }

    /// ST(ESC \) 대기 중: '\\'면 시퀀스 완료, 아니면 ESC+b를 본문으로 되돌린다.
    fn collect_st(&mut self, b: u8, body_state: u8) {
        if b == b'\\' {
            self.finish_seq();
            self.escan.state = 0;
        } else {
            if self.escan.buf.len() + 2 < MAX_BUF && !self.escan.osc_skip {
                self.escan.buf.push(0x1B);
                self.escan.buf.push(b);
            }
            self.escan.state = body_state;
        }
    }

    /// OSC 본문: BEL 종료 / ESC면 ST 대기. 1337;File= 아니면 버퍼링 생략.
    fn osc_body(&mut self, b: u8) {
        match b {
            0x07 => {
                self.finish_seq();
                self.escan.state = 0;
            }
            0x1B => self.escan.state = 5,
            _ => {
                if !self.escan.osc_skip && self.escan.buf.len() < MAX_BUF {
                    self.escan.buf.push(b);
                    let t = b"1337;File=";
                    if self.escan.buf.len() <= t.len()
                        && self.escan.buf[..] != t[..self.escan.buf.len()]
                    {
                        self.escan.osc_skip = true;
                        self.escan.buf.clear();
                    }
                }
            }
        }
    }

    /// 완성된 시퀀스를 종류별로 디코드·배치한다.
    fn finish_seq(&mut self) {
        let buf = std::mem::take(&mut self.escan.buf);
        match self.escan.kind {
            2 => {
                if let Some(img) = nabi_image::decode_sixel(&buf) {
                    self.place_decoded(img);
                }
            }
            4 => {
                if !self.escan.osc_skip {
                    if let Some(img) = nabi_image::decode_iterm_osc(&buf) {
                        self.place_decoded(img);
                    }
                }
            }
            _ => self.handle_kitty(&buf),
        }
    }

    /// Kitty APC(`G <k=v,...>;<base64>`) — 청크 누적 후 m=0에서 디코드·배치.
    fn handle_kitty(&mut self, body: &[u8]) {
        if body.first() != Some(&b'G') {
            return;
        }
        let rest = &body[1..];
        let semi = rest.iter().position(|&b| b == b';').unwrap_or(rest.len());
        let payload = rest.get(semi + 1..).unwrap_or(&[]);
        let (mut f, mut w, mut h, mut m, mut a) = (32u32, 0u32, 0u32, 0u32, b'T');
        for kv in rest[..semi].split(|&b| b == b',') {
            let Some(eq) = kv.iter().position(|&b| b == b'=') else {
                continue;
            };
            let v = &kv[eq + 1..];
            match &kv[..eq] {
                b"f" => f = parse_u32(v),
                b"s" => w = parse_u32(v),
                b"v" => h = parse_u32(v),
                b"m" => m = parse_u32(v),
                b"a" => a = v.first().copied().unwrap_or(b'T'),
                _ => {}
            }
        }
        match self.kitty.as_mut() {
            None => {
                self.kitty = Some(KittyPending { fmt: f, w, h, b64: payload.to_vec(), display: a == b'T' });
            }
            Some(p) if p.b64.len() < MAX_BUF => p.b64.extend_from_slice(payload),
            _ => {}
        }
        if m == 0 {
            if let Some(p) = self.kitty.take() {
                if p.display {
                    if let Some(raw) = nabi_image::base64_decode(&p.b64) {
                        if let Some(img) = nabi_image::kitty_assemble(p.fmt, p.w, p.h, &raw) {
                            self.place_decoded(img);
                        }
                    }
                }
            }
        }
    }

    /// 디코드된 이미지를 커서 위치에 배치하고 높이만큼 줄을 확보한다.
    fn place_decoded(&mut self, img: DecodedImage) {
        let rows = ((img.height as f32 / self.cell_px_h).ceil() as i64).clamp(1, 300) as u16;
        let anchor_abs = self.history_size() as i64 + i64::from(self.cursor_line());
        let id = self.next_img_id;
        self.next_img_id += 1;
        self.images.push(PlacedImage { id, anchor_abs, rows, img: Arc::new(img) });
        while self.images.len() > 64 {
            self.images.remove(0);
        }
        for _ in 0..rows {
            self.feed_parser(b'\r');
            self.feed_parser(b'\n');
        }
    }

    /// 현재 화면과 겹치는 이미지들: (상단 행(음수 가능), 행수, id, 이미지).
    pub fn visible_images(&self) -> Vec<(i32, u16, u64, Arc<DecodedImage>)> {
        let h = self.history_size() as i64;
        let d = self.scrollback_offset() as i64;
        let gr = i64::from(self.size().rows());
        self.images
            .iter()
            .filter_map(|p| {
                let top = p.anchor_abs - h + d;
                let bot = top + i64::from(p.rows);
                (bot > 0 && top < gr).then_some((top as i32, p.rows, p.id, p.img.clone()))
            })
            .collect()
    }
}

/// 바이트열 숫자를 u32로(비숫자는 무시).
fn parse_u32(v: &[u8]) -> u32 {
    let mut n: u32 = 0;
    for &b in v {
        if b.is_ascii_digit() {
            n = n.saturating_mul(10).saturating_add(u32::from(b - b'0'));
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use crate::TermModel;
    use nabi_types::GridSize;

    #[test]
    fn places_sixel_image_and_reserves_rows() {
        let mut m = TermModel::new(GridSize::new(40, 20), 200);
        m.set_cell_px(10.0);
        m.process(b"\x1bPq#0;2;100;0;0~\x1b\\");
        let imgs = m.visible_images();
        assert_eq!(imgs.len(), 1);
        assert_eq!((imgs[0].3.width, imgs[0].3.height), (1, 6));
        assert_eq!(imgs[0].0, 0);
    }

    #[test]
    fn non_sixel_dcs_is_ignored() {
        let mut m = TermModel::new(GridSize::new(40, 10), 100);
        m.process(b"\x1bP$q\"p\x1b\\");
        assert!(m.visible_images().is_empty());
    }

    #[test]
    fn places_kitty_rgb_image() {
        let mut m = TermModel::new(GridSize::new(40, 20), 200);
        m.set_cell_px(10.0);
        // 2x1 RGB(빨강,초록) base64="/wAAAP8A" → a=T,f=24,s=2,v=1.
        let b64 = nabi_image::base64_decode(b"/wAAAP8A"); // 역검증용(사용 안 함).
        let _ = b64;
        m.process(b"\x1b_Ga=T,f=24,s=2,v=1;/wAAAP8A\x1b\\");
        let imgs = m.visible_images();
        assert_eq!(imgs.len(), 1);
        assert_eq!((imgs[0].3.width, imgs[0].3.height), (2, 1));
    }

    #[test]
    fn ignores_non_image_osc() {
        let mut m = TermModel::new(GridSize::new(40, 10), 100);
        m.process(b"\x1b]0;some title\x07"); // OSC 0(제목) — 이미지 아님.
        assert!(m.visible_images().is_empty());
    }
}
