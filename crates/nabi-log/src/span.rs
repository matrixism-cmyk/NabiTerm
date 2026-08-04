//! pane/세션 단위 tracing span 헬퍼.

use nabi_types::PaneId;

/// pane 단위 span을 만든다(로그를 pane별로 구분).
pub fn pane_span(pane: PaneId) -> tracing::Span {
    tracing::info_span!("pane", id = pane.get())
}
