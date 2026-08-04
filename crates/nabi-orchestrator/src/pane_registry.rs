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

/// 오염된 잠금을 복구해 읽는다.
///
/// 어느 스레드든 잠금을 든 채 패닉하면 `RwLock`이 오염되고, 이후 `unwrap()`은 **모두** 패닉한다.
/// 오케스트레이터 스레드가 그렇게 죽으면 창은 그려지는데 아무 반응 없는 좀비가 된다.
/// pane 맵은 단순 컨테이너라 패닉 시점의 논리적 불변식이 깨질 여지가 없으므로 복구해 계속 쓴다.
/// (actor.rs의 catch_unwind 패닉 격리가 실제로 의미를 가지려면 이 복구가 필요하다.)
pub fn panes_read(p: &SharedPanes) -> std::sync::RwLockReadGuard<'_, HashMap<PaneId, PaneView>> {
    p.read().unwrap_or_else(|e| e.into_inner())
}

/// 오염된 잠금을 복구해 쓴다. 근거는 [`panes_read`] 참조.
pub fn panes_write(p: &SharedPanes) -> std::sync::RwLockWriteGuard<'_, HashMap<PaneId, PaneView>> {
    p.write().unwrap_or_else(|e| e.into_inner())
}

/// 오염된 pane 모델 잠금을 복구해 잠근다.
///
/// 모델이 오염되면 기존 코드는 `if let Ok(..)`로 **조용히 건너뛰어**, 그 pane이 입력은 받지만
/// 출력이 영영 안 나오는 상태가 됐다. VT 모델은 자체적으로 일관성을 복구하므로 이어서 쓴다.
pub fn model_lock(m: &SharedModel) -> std::sync::MutexGuard<'_, TermModel> {
    m.lock().unwrap_or_else(|e| e.into_inner())
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

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_types::{next_pane_id, GridSize};

    fn view() -> PaneView {
        let model = Arc::new(Mutex::new(TermModel::new(GridSize::new(80, 24), 100)));
        PaneView::new(model, "t".into(), "local")
    }

    /// 잠금을 든 채 패닉해 오염시킨 뒤에도 읽기/쓰기가 계속 동작해야 한다.
    /// (오염 시 unwrap하면 오케스트레이터 스레드가 죽어 앱이 좀비가 된다.)
    #[test]
    fn recovers_from_poisoned_panes_lock() {
        let panes = new_shared_panes();
        let id = next_pane_id();
        panes_write(&panes).insert(id, view());

        // 쓰기 잠금을 든 채 패닉 → RwLock 오염.
        let p = panes.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = panes_write(&p);
            panic!("보유 중 패닉");
        }));
        assert!(panes.read().is_err(), "이 시점에 잠금은 오염 상태여야 한다");

        // 복구 헬퍼는 계속 동작한다.
        assert!(panes_read(&panes).contains_key(&id), "오염 후에도 읽기 가능");
        panes_write(&panes).remove(&id);
        assert!(panes_read(&panes).is_empty(), "오염 후에도 쓰기 가능");
    }

    /// 모델 잠금이 오염돼도 출력 처리가 이어져야 한다(과거엔 조용히 건너뛰어 pane이 멈췄다).
    #[test]
    fn recovers_from_poisoned_model_lock() {
        let v = view();
        let m = v.model.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = model_lock(&m);
            panic!("모델 잠금 보유 중 패닉");
        }));
        assert!(v.model.lock().is_err(), "모델 잠금이 오염 상태여야 한다");
        model_lock(&v.model).process(b"hello"); // 패닉하지 않고 처리되면 성공.
    }
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
