//! nabi-render — 터미널 그리드 렌더 + 키 입력 변환.
//!
//! M1은 egui Painter로 셀 그리드를 그린다(CPU). 구조는 트레잇 없이 단순하지만,
//! 후속에 egui_wgpu CallbackTrait 기반 GPU 렌더러로 교체할 수 있게 페인트 진입점을
//! 한 함수로 모았다.

mod findhl;
pub mod smartcase;
mod glyphcache;
mod highlight;
mod underline;
pub mod input;
mod keymap;
mod kittykeys;
mod overlay;
pub mod mouse;
pub mod painter;
pub mod paste;
pub mod pastedeceive;
pub mod pastepreview;
pub mod urlrules;
pub mod urls;
mod urlspath;
mod fileref;
mod urls_email;
mod urls_pytrace;
mod urls_scp;
mod screenurls;
pub mod title;

pub use input::{events_to_bytes, events_to_bytes_kitty, update_preedit};
pub use keymap::key_label;
pub use mouse::{mouse_report, MouseBtn};
pub use painter::{cell_size, paint};
pub use paste::sanitize_paste;
pub use fileref::parse_file_ref;
pub use screenurls::{screen_urls, with_screen_urls, ScreenUrl};
pub use title::{clean_title, ellipsize_middle};
pub use urls::{row_urls, UrlSpan};
