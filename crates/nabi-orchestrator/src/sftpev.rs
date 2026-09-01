//! SFTP 결과를 이벤트로 옮기는 작은 변환들 — sftp.rs가 소프트 라인 한도에 닿아 분리했다.
//!
//! 전부 순수 변환이라 요청 루프와 섞여 있을 이유가 없었다.

use nabi_fs::FileKind;
use nabi_proto::{Event, SftpEntry, SftpId};

/// 전송 결과를 완료 이벤트로 변환(큐 항목 `xfer`에 귀속).
pub(crate) fn transfer_done(id: SftpId, xfer: u64, name: &str, res: Result<(), String>) -> Event {
    Event::SftpTransferDone {
        id,
        xfer,
        name: name.to_string(),
        ok: res.is_ok(),
        message: res.err().unwrap_or_default(),
    }
}

/// 파일 작업(삭제·이름변경·권한 등) 결과 — 전송 큐와 무관한 별도 이벤트.
pub(crate) fn op_done(id: SftpId, name: &str, res: Result<(), String>) -> Event {
    Event::SftpOpDone {
        id,
        name: name.to_string(),
        ok: res.is_ok(),
        message: res.err().unwrap_or_default(),
    }
}

/// nabi-fs FileEntry → proto SftpEntry(디렉터리 여부 평탄화).
pub(crate) fn to_entry(e: nabi_fs::FileEntry) -> SftpEntry {
    SftpEntry {
        name: e.name,
        // 폴더 링크는 **둘 다** 참이다 — 들어갈 수 있고(is_dir), 링크로 보인다(is_link).
        is_dir: matches!(e.kind, FileKind::Dir | FileKind::LinkDir),
        is_link: matches!(e.kind, FileKind::Symlink | FileKind::LinkDir),
        size: e.size,
        mode: e.mode,
        mtime: e.mtime,
        uid: e.uid,
        gid: e.gid,
    }
}
