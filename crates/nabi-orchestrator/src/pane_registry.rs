//! pane 레지스트리 타입.
//!
//! - `SharedPanes`: UI가 읽는 공유 뷰(모델 + 제목).
//! - `PaneRuntime`: 오케스트레이터만 소유하는 런타임(전송 채널 + OSC 탭).

use nabi_osc::OscScanner;
use nabi_pty::ByteChannel;
use nabi_types::PaneId;
use nabi_vt::TermModel;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// UI 렌더가 락을 잡고 읽는 pane 화면 모델.
pub type SharedModel = Arc<Mutex<TermModel>>;

/// UI가 읽는 pane 뷰.
#[derive(Clone)]
pub struct PaneView {
    pub model: SharedModel,
    pub title: String,
    /// 사용자/제어 평면이 지정한 제목 — OSC 제목보다 우선(set-title·--match 타깃).
    pub user_title: Arc<Mutex<Option<String>>>,
    /// 메타데이터·활동 상태(CP-6 — 제어 평면 list가 질의).
    pub meta: Arc<Mutex<crate::pane_meta::PaneMeta>>,
}

impl PaneView {
    /// 모델·제목·종류로 뷰를 만든다(meta는 새로 초기화).
    pub fn new(model: SharedModel, title: String, kind: &'static str) -> Self {
        Self {
            model,
            title,
            user_title: Arc::new(Mutex::new(None)),
            meta: Arc::new(Mutex::new(crate::pane_meta::PaneMeta::new(kind))),
        }
    }
}

/// PaneId → 뷰. UI와 오케스트레이터가 공유한다(읽기 위주).
pub type SharedPanes = Arc<RwLock<HashMap<PaneId, PaneView>>>;

/// 빈 공유 레지스트리를 만든다.
pub fn new_shared_panes() -> SharedPanes {
    Arc::new(RwLock::new(HashMap::new()))
}

/// 오케스트레이터 스레드만 소유하는 pane 런타임.
pub struct PaneRuntime {
    pub transport: Box<dyn ByteChannel>,
    pub osc: OscScanner,
    /// 비UTF-8 입력 디코더(UTF-8이면 None = 원본 통과).
    pub decoder: Option<encoding_rs::Decoder>,
}

/// 인코딩 라벨("UTF-8","EUC-KR","Shift_JIS"…)로 디코더 생성. UTF-8이면 None.
pub fn decoder_for(label: &str) -> Option<encoding_rs::Decoder> {
    let enc = encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    (enc != encoding_rs::UTF_8).then(|| enc.new_decoder())
}

impl PaneRuntime {
    /// 청크를 pane 인코딩으로 디코드(UTF-8이면 None=원본 사용). 스트리밍(부분 시퀀스 보존).
    pub fn decode(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
        let dec = self.decoder.as_mut()?;
        let mut out = String::with_capacity(bytes.len() + 16);
        let _ = dec.decode_to_string(bytes, &mut out, false);
        Some(out.into_bytes())
    }

    /// 런타임 인코딩 변경(SetEncoding).
    pub fn set_encoding(&mut self, label: &str) {
        self.decoder = decoder_for(label);
    }
}
