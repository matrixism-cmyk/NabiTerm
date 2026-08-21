//! SFTP 원격 폴더 → 탐색기 드래그-아웃. 파일과 달리 폴더는 드롭 시 전체를 임시폴더로
//! 재귀 다운로드(SftpDownloadDirSync, 완료를 done 채널로 동기 수신)한 뒤 그 폴더 경로를
//! CF_HDROP로 제공한다(탐색기가 폴더 통째로 복사). ⚠️ 수동 테스트 필요.
#![allow(non_snake_case)]

use crossbeam_channel::Sender;
use nabi_proto::{Command, SftpId};
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use windows::core::{implement, Result, HRESULT};
use windows::Win32::Foundation::{
    BOOL, DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS,
    DV_E_FORMATETC, E_FAIL, E_NOTIMPL, OLE_E_ADVISENOTSUPPORTED, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DVASPECT_CONTENT,
    FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;

#[implement(IDataObject)]
struct VirtFolder {
    name: String,
    remote_path: String,
    sftp_id: SftpId,
    cmd_tx: Sender<Command>,
}

fn is_hdrop(f: &FORMATETC) -> bool {
    f.cfFormat == CF_HDROP.0 && f.dwAspect == DVASPECT_CONTENT.0 && (f.tymed & TYMED_HGLOBAL.0 as u32) != 0
}

impl VirtFolder {
    /// 드롭 시: 임시폴더로 재귀 다운로드(동기 완료 대기) → 그 폴더의 CF_HDROP.
    fn hdrop(&self) -> Result<STGMEDIUM> {
        static N: AtomicU64 = AtomicU64::new(1);
        let sub = N.fetch_add(1, Ordering::Relaxed);
        // temp/nabi-dnd-<n>/<폴더명> — 드롭된 폴더가 원래 이름으로 복사되게.
        let temp = std::env::temp_dir().join(format!("nabi-dnd-{sub}")).join(&self.name);
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = self.cmd_tx.send(Command::SftpDownloadDirSync {
            id: self.sftp_id,
            remote: self.remote_path.clone(),
            local: temp.to_string_lossy().into_owned(),
            done: tx,
        });
        if !matches!(rx.recv_timeout(Duration::from_secs(600)), Ok(true)) {
            return Err(E_FAIL.into());
        }
        let hg = crate::winclip::build_hdrop(&[temp]).ok_or(windows::core::Error::from(E_FAIL))?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hg },
            pUnkForRelease: ManuallyDrop::new(None),
        })
    }
}

impl IDataObject_Impl for VirtFolder_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> Result<STGMEDIUM> {
        // SAFETY: OLE 런타임이 IDataObject 계약에 따라 호출 동안 유효한 FORMATETC를 준다.
        // 참조를 이 호출 밖으로 내보내지 않는다.
        if is_hdrop(unsafe { &*pformatetcin }) {
            self.hdrop()
        } else {
            Err(DV_E_FORMATETC.into())
        }
    }
    fn GetDataHere(&self, _f: *const FORMATETC, _m: *mut STGMEDIUM) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        // SAFETY: OLE 런타임이 IDataObject 계약에 따라 호출 동안 유효한 FORMATETC를 준다.
        // 참조를 이 호출 밖으로 내보내지 않는다.
        if is_hdrop(unsafe { &*pformatetc }) {
            S_OK
        } else {
            S_FALSE
        }
    }
    fn GetCanonicalFormatEtc(&self, _i: *const FORMATETC, pout: *mut FORMATETC) -> HRESULT {
        if !pout.is_null() {
            // SAFETY: 바로 위에서 null이 아님을 확인했고, OLE가 준 out 파라미터는 호출 동안 쓰기 가능하다.
            unsafe { (*pout).ptd = std::ptr::null_mut() };
        }
        DATA_S_SAMEFORMATETC
    }
    fn SetData(&self, _f: *const FORMATETC, _m: *const STGMEDIUM, _r: BOOL) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn EnumFormatEtc(&self, _dir: u32) -> Result<IEnumFORMATETC> {
        let fmt = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        // SAFETY: 슬라이스는 포인터+길이로 함께 전달되고, 셸이 내용을 복사해 열거자를 만든다.
        unsafe { SHCreateStdEnumFmtEtc(&[fmt]) }
    }
    fn DAdvise(&self, _f: *const FORMATETC, _a: u32, _s: Option<&IAdviseSink>) -> Result<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
    fn DUnadvise(&self, _c: u32) -> Result<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
    fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}

#[implement(IDropSource)]
struct FolderSrc;

impl IDropSource_Impl for FolderSrc_Impl {
    fn QueryContinueDrag(&self, esc: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if esc.as_bool() {
            DRAGDROP_S_CANCEL
        } else if (keys & MK_LBUTTON).0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }
    fn GiveFeedback(&self, _e: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// 원격 폴더를 가상 폴더로 드래그한다(블로킹). 복사 드롭되면 true.
pub(crate) fn drag_out_remote_dir(
    name: String,
    remote_path: String,
    sftp_id: SftpId,
    cmd_tx: Sender<Command>,
) -> bool {
    let data: IDataObject = VirtFolder { name, remote_path, sftp_id, cmd_tx }.into();
    let src: IDropSource = FolderSrc.into();
    let mut effect = DROPEFFECT::default();
    // SAFETY: data·src는 이 함수가 잡고 있는 COM 객체라 호출이 끝날 때까지 살아 있다.
    // effect는 스택 지역의 가변 참조다. OLE 초기화는 init_ole이 보장한다.
    let hr = unsafe { DoDragDrop(&data, &src, DROPEFFECT_COPY, &mut effect) };
    hr == DRAGDROP_S_DROP
}
