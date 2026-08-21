//! trzsz(`trz`/`tsz`) 파일 전송에 쓰는 UI↔오케스트레이터 타입.
//!
//! 프로토콜 자체는 nabi-trzsz가 안다. 여기 있는 것은 **사용자에게 물어볼 것**과
//! **화면에 보여줄 것**뿐이다. `Storage` 같은 트레이트 객체는 스레드를 건너지 않는다 —
//! 사용자의 결정(경로·파일 목록)만 넘기고 실제 파일 입출력은 오케스트레이터가 만든다.

use nabi_types::PaneId;
use std::path::PathBuf;

/// 원격이 요청한 전송의 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XferMode {
    /// `tsz` — 원격이 보낸다(우리가 받는다).
    Download,
    /// `trz` — 우리가 보낸다.
    Upload,
    /// `trz -d` — 폴더까지 보낼 수 있다.
    UploadDir,
    /// 원격이 **올릴 로컬 파일을 지정**한다. 기본 차단(§보안).
    UploadSpecified,
}

impl XferMode {
    pub fn is_upload(self) -> bool {
        !matches!(self, Self::Download)
    }
}

/// 사용자의 결정.
#[derive(Debug, Clone)]
pub struct XferDecision {
    pub pane: PaneId,
    pub accept: bool,
    /// 다운로드일 때 저장할 폴더.
    pub save_dir: Option<PathBuf>,
    /// 업로드일 때 보낼 파일들.
    pub upload: Vec<PathBuf>,
}

impl XferDecision {
    pub fn reject(pane: PaneId) -> Self {
        Self { pane, accept: false, save_dir: None, upload: Vec::new() }
    }

    pub fn download_to(pane: PaneId, dir: PathBuf) -> Self {
        Self { pane, accept: true, save_dir: Some(dir), upload: Vec::new() }
    }

    pub fn upload(pane: PaneId, files: Vec<PathBuf>) -> Self {
        Self { pane, accept: true, save_dir: None, upload: files }
    }
}

/// 진행 상황(막대·속도는 UI가 계산한다).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XferProgress {
    /// 몇 번째 파일인가(1부터).
    pub index: usize,
    pub count: usize,
    pub name: String,
    pub done: u64,
    pub total: u64,
}
