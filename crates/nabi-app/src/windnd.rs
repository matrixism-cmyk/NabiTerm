//! 탐색기로 끌어내기(OLE 드래그-아웃) — 로컬 파일/폴더를 CF_HDROP IDataObject로
//! 제공하고 DoDragDrop으로 OS 드래그를 시작한다. 탐색기에 놓으면 복사.
//!
//! ⚠️ DoDragDrop은 드롭/취소까지 블로킹(자체 모달 루프)한다. egui 프레임 중 호출되면
//! 그 동안 UI는 멈추지만 OS가 드래그 이미지를 그린다(드래그 동작상 정상).

#![allow(non_snake_case)]

use std::path::PathBuf;
use windows::core::{implement, Result, HRESULT};
use windows::Win32::Foundation::{
    BOOL, DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS,
    DV_E_FORMATETC, E_NOTIMPL, OLE_E_ADVISENOTSUPPORTED, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DVASPECT_CONTENT,
    FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Ole::{
    DoDragDrop, OleInitialize, IDropSource, IDropSource_Impl, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY,
};

/// eframe 창의 HWND(isize). OS 파일 드롭 위치 판정·드래그에 사용.
pub(crate) fn hwnd_of(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    cc.window_handle().ok().and_then(|h| match h.as_raw() {
        RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
        _ => None,
    })
}

/// OLE 초기화(드래그-아웃에 필요). winit도 호출하나 방어적으로 한 번 더(멱등).
pub(crate) fn init_ole() {
    unsafe {
        let _ = OleInitialize(None);
    }
}

/// 현재 커서의 창 클라이언트 좌표(물리 px). OS 파일 드롭 위치 판정용.
pub(crate) fn cursor_client_px(hwnd: isize) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        let mut p = POINT::default();
        GetCursorPos(&mut p).ok()?;
        let h = HWND(hwnd as *mut core::ffi::c_void);
        if ScreenToClient(h, &mut p).as_bool() {
            Some((p.x, p.y))
        } else {
            None
        }
    }
}
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;

/// CF_HDROP(파일 목록) 한 가지 포맷만 제공하는 데이터 객체.
#[implement(IDataObject)]
struct FileData {
    paths: Vec<PathBuf>,
}

/// 우리 포맷(CF_HDROP / DVASPECT_CONTENT / TYMED_HGLOBAL)인지.
fn is_hdrop(f: &FORMATETC) -> bool {
    f.cfFormat == CF_HDROP.0
        && (f.dwAspect == DVASPECT_CONTENT.0)
        && (f.tymed & TYMED_HGLOBAL.0 as u32) != 0
}

impl IDataObject_Impl for FileData_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> Result<STGMEDIUM> {
        let f = unsafe { &*pformatetcin };
        if !is_hdrop(f) {
            return Err(DV_E_FORMATETC.into());
        }
        // 호출측(드롭 타깃)이 ReleaseStgMedium으로 해제하므로 매번 새 HGLOBAL을 만든다.
        let hg = crate::winclip::build_hdrop(&self.paths).ok_or(windows::core::Error::from(S_FALSE))?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hg },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        })
    }

    fn GetDataHere(&self, _f: *const FORMATETC, _m: *mut STGMEDIUM) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        if is_hdrop(unsafe { &*pformatetc }) {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn GetCanonicalFormatEtc(&self, _i: *const FORMATETC, pout: *mut FORMATETC) -> HRESULT {
        if !pout.is_null() {
            unsafe { (*pout).ptd = std::ptr::null_mut() };
        }
        DATA_S_SAMEFORMATETC
    }

    fn SetData(&self, _f: *const FORMATETC, _m: *const STGMEDIUM, _r: BOOL) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn EnumFormatEtc(&self, _dwdirection: u32) -> Result<IEnumFORMATETC> {
        // 표준 열거자 헬퍼로 우리 한 포맷을 노출(직접 IEnumFORMATETC 구현 회피).
        let fmt = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        unsafe { SHCreateStdEnumFmtEtc(&[fmt]) }
    }

    fn DAdvise(&self, _f: *const FORMATETC, _advf: u32, _s: Option<&IAdviseSink>) -> Result<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
    fn DUnadvise(&self, _c: u32) -> Result<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
    fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}

/// 드래그 소스: 왼쪽 버튼을 놓으면 드롭, Esc면 취소.
#[implement(IDropSource)]
struct DropSrc;

impl IDropSource_Impl for DropSrc_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        if fescapepressed.as_bool() {
            DRAGDROP_S_CANCEL
        } else if (grfkeystate & MK_LBUTTON).0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }
    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// 주어진 로컬 경로들로 OS 드래그를 시작한다(블로킹). 복사로 드롭되면 true.
pub(crate) fn drag_out(paths: &[PathBuf]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let data: IDataObject = FileData { paths: paths.to_vec() }.into();
    let src: IDropSource = DropSrc.into();
    let mut effect = DROPEFFECT::default();
    let hr = unsafe { DoDragDrop(&data, &src, DROPEFFECT_COPY, &mut effect) };
    hr == DRAGDROP_S_DROP
}
