//! 탐색기 연동 클립보드(CF_HDROP) — 로컬 파일 복사/붙여넣기.
//!
//! nabi 브라우저에서 "복사"(Ctrl+C)하면 CF_HDROP로 클립보드에 올려 탐색기에서
//! Ctrl+V로 붙여넣을 수 있고, 탐색기에서 복사한 파일을 nabi에서 "붙여넣기"하면
//! 현재 폴더로 복사한다. (드래그-아웃·SFTP 가상 파일은 후속 단계.)

use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::UI::Shell::{DragQueryFileW, DROPFILES, HDROP};

/// 경로들을 CF_HDROP HGLOBAL로 만든다(클립보드·드래그 공용). 실패 시 None.
/// 호출측이 소유권을 가진다(클립보드/STGMEDIUM에 넘기거나 GlobalFree).
pub(crate) fn build_hdrop(paths: &[PathBuf]) -> Option<windows::Win32::Foundation::HGLOBAL> {
    if paths.is_empty() {
        return None;
    }
    // 경로 목록: 각 경로 뒤 NUL, 마지막에 NUL 하나 더(이중 NUL 종결).
    let mut list: Vec<u16> = Vec::new();
    for p in paths {
        list.extend(p.as_os_str().encode_wide());
        list.push(0);
    }
    list.push(0);
    let df = std::mem::size_of::<DROPFILES>();
    let bytes = df + list.len() * 2;
    // SAFETY: GlobalAlloc이 준 bytes 크기 블록에만 쓴다 — DROPFILES 헤더(df) + 목록(list)이
    // 정확히 bytes다. ptr이 null이면 쓰기 전에 반환하고, 복사량도 list.len()로 제한한다.
    unsafe {
        let hg = GlobalAlloc(GMEM_MOVEABLE, bytes).ok()?;
        let ptr = GlobalLock(hg);
        if ptr.is_null() {
            return None;
        }
        // DROPFILES 헤더(파일 목록 오프셋 + 와이드 문자 플래그).
        let h = ptr as *mut DROPFILES;
        (*h).pFiles = df as u32;
        (*h).fWide = true.into();
        std::ptr::copy_nonoverlapping(list.as_ptr(), (ptr as *mut u8).add(df) as *mut u16, list.len());
        let _ = GlobalUnlock(hg);
        Some(hg)
    }
}

/// 로컬 경로들을 CF_HDROP로 클립보드에 올린다(탐색기가 붙여넣기로 복사).
pub(crate) fn copy_paths(paths: &[PathBuf]) -> bool {
    let Some(hg) = build_hdrop(paths) else {
        return false;
    };
    // SAFETY: OpenClipboard 성공을 확인한 뒤에만 이어 간다. SetClipboardData가 성공하면
    // hglobal 소유권이 클립보드로 넘어가므로 우리가 해제하지 않는다(이중 해제 없음).
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        // 성공 시 클립보드가 hglobal 소유권을 가져간다(해제하지 않음).
        let ok = SetClipboardData(CF_HDROP.0 as u32, HANDLE(hg.0)).is_ok();
        let _ = CloseClipboard();
        ok
    }
}

/// 클립보드의 CF_HDROP 파일 목록을 읽는다(탐색기에서 복사한 파일 → nabi 붙여넣기).
pub(crate) fn paste_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    // SAFETY: 클립보드는 열기→읽기→닫기를 이 블록 안에서 끝낸다. DragQueryFileW에 넘기는
    // 버퍼는 같은 호출이 알려 준 길이로 미리 잡고, 실패 시 조기 반환한다.
    unsafe {
        if OpenClipboard(None).is_err() {
            return out;
        }
        if let Ok(h) = GetClipboardData(CF_HDROP.0 as u32) {
            let hdrop = HDROP(h.0);
            let n = DragQueryFileW(hdrop, u32::MAX, None);
            for i in 0..n {
                let len = DragQueryFileW(hdrop, i, None) as usize;
                if len == 0 {
                    continue;
                }
                let mut buf = vec![0u16; len + 1];
                let got = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
                buf.truncate(got);
                out.push(PathBuf::from(std::ffi::OsString::from_wide(&buf)));
            }
        }
        let _ = CloseClipboard();
    }
    out
}
