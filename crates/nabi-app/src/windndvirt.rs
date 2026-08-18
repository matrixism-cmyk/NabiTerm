//! SFTP 원격 파일 → 탐색기 드래그-아웃(가상 파일, 드롭 시 다운로드).
//!
//! 원격 파일은 로컬 실파일이 없으므로 CFSTR_FILEDESCRIPTORW(이름·크기) +
//! CFSTR_FILECONTENTS(IStream)를 제공한다. 탐색기가 드롭 시 FILECONTENTS를 읽을 때
//! 비로소 임시폴더로 SFTP 다운로드하고(오케스트레이터에 SftpDownload 명령), 파일 크기가
//! 채워질 때까지 폴링한 뒤 그 임시파일의 IStream을 돌려준다(지연 렌더링).
//!
//! ⚠️ 드래그는 마우스 조작이라 자동 검증 불가 — 수동 테스트 필요.
#![allow(non_snake_case)]

use crossbeam_channel::Sender;
use nabi_proto::{Command, SftpId};
use std::time::{Duration, Instant};
use windows::core::{implement, Result, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    BOOL, DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS,
    DV_E_FORMATETC, E_FAIL, E_NOTIMPL, OLE_E_ADVISENOTSUPPORTED, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, IStream,
    DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0, STGM_READ, STGM_SHARE_DENY_WRITE,
    TYMED_HGLOBAL, TYMED_ISTREAM,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, DROPEFFECT, DROPEFFECT_COPY,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{
    SHCreateStdEnumFmtEtc, SHCreateStreamOnFileEx, FD_FILESIZE, FILEDESCRIPTORW,
    FILEGROUPDESCRIPTORW,
};

/// 등록된 클립보드 포맷 ID(런타임 등록).
fn cf(name: PCWSTR) -> u16 {
    unsafe { RegisterClipboardFormatW(name) as u16 }
}
fn cf_descriptor() -> u16 {
    cf(windows::core::w!("FileGroupDescriptorW"))
}
fn cf_contents() -> u16 {
    cf(windows::core::w!("FileContents"))
}

/// 드래그할 원격 파일 1개(이름·크기·원격 경로).
#[derive(Clone)]
pub(crate) struct RemoteFile {
    pub name: String,
    pub size: u64,
    pub remote_path: String,
}

#[implement(IDataObject)]
struct VirtData {
    file: RemoteFile,
    sftp_id: SftpId,
    cmd_tx: Sender<Command>,
}

impl VirtData {
    /// FILEGROUPDESCRIPTORW(파일 1개) HGLOBAL.
    fn descriptor(&self) -> Result<STGMEDIUM> {
        let bytes = std::mem::size_of::<FILEGROUPDESCRIPTORW>();
        unsafe {
            let hg = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|_| windows::core::Error::from(E_FAIL))?;
            let p = GlobalLock(hg) as *mut FILEGROUPDESCRIPTORW;
            if p.is_null() {
                return Err(E_FAIL.into());
            }
            (*p).cItems = 1;
            let fd: &mut FILEDESCRIPTORW = &mut (*p).fgd[0];
            *fd = std::mem::zeroed();
            fd.dwFlags = FD_FILESIZE.0 as u32;
            fd.nFileSizeLow = self.file.size as u32;
            fd.nFileSizeHigh = (self.file.size >> 32) as u32;
            let wname: Vec<u16> = self.file.name.encode_utf16().take(259).collect();
            for (i, &c) in wname.iter().enumerate() {
                fd.cFileName[i] = c;
            }
            let _ = GlobalUnlock(hg);
            Ok(STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                u: STGMEDIUM_0 { hGlobal: hg },
                pUnkForRelease: std::mem::ManuallyDrop::new(None),
            })
        }
    }

    /// 드롭 시 호출: 임시폴더로 SFTP 다운로드 후 그 파일의 IStream 반환(지연 렌더링).
    fn contents(&self) -> Result<STGMEDIUM> {
        let temp = std::env::temp_dir().join(format!("nabi-dnd-{}", self.file.name));
        let _ = std::fs::remove_file(&temp);
        let _ = self.cmd_tx.send(Command::SftpDownload {
            id: self.sftp_id,
            xfer: crate::sftpxfer::XFER_NONE, // 큐에 표시하지 않는 내부 전송(드래그아웃).
            remote: self.file.remote_path.clone(),
            local: temp.to_string_lossy().into_owned(),
            resume: 0,
        });
        if !wait_download(&temp, self.file.size) {
            return Err(E_FAIL.into());
        }
        let mut wide: Vec<u16> = temp.to_string_lossy().encode_utf16().collect();
        wide.push(0);
        let stream: IStream = unsafe {
            SHCreateStreamOnFileEx(
                PCWSTR(wide.as_ptr()),
                (STGM_READ | STGM_SHARE_DENY_WRITE).0,
                0,
                false,
                None,
            )?
        };
        Ok(STGMEDIUM {
            tymed: TYMED_ISTREAM.0 as u32,
            u: STGMEDIUM_0 {
                pstm: std::mem::ManuallyDrop::new(Some(stream)),
            },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        })
    }
}

/// 임시파일이 원격 크기만큼 채워질 때까지 폴링(완료 감지). 타임아웃 120초.
fn wait_download(temp: &std::path::Path, size: u64) -> bool {
    let start = Instant::now();
    let mut last = 0u64;
    let mut stable = 0;
    while start.elapsed() < Duration::from_secs(120) {
        let cur = std::fs::metadata(temp).map(|m| m.len()).unwrap_or(0);
        if size > 0 && cur >= size {
            return true;
        }
        // 크기 미상(0)이면 2회 연속 같은 크기 + 존재 시 완료로 간주.
        if size == 0 && cur > 0 {
            if cur == last {
                stable += 1;
                if stable >= 3 {
                    return true;
                }
            } else {
                stable = 0;
            }
        }
        last = cur;
        std::thread::sleep(Duration::from_millis(60));
    }
    size > 0 && std::fs::metadata(temp).map(|m| m.len()).unwrap_or(0) >= size
}

fn is_fmt(f: &FORMATETC, cfmt: u16, tymed: u32) -> bool {
    f.cfFormat == cfmt && f.dwAspect == DVASPECT_CONTENT.0 && (f.tymed & tymed) != 0
}

impl IDataObject_Impl for VirtData_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> Result<STGMEDIUM> {
        let f = unsafe { &*pformatetcin };
        if is_fmt(f, cf_descriptor(), TYMED_HGLOBAL.0 as u32) {
            self.descriptor()
        } else if is_fmt(f, cf_contents(), TYMED_ISTREAM.0 as u32) {
            self.contents()
        } else {
            Err(DV_E_FORMATETC.into())
        }
    }
    fn GetDataHere(&self, _f: *const FORMATETC, _m: *mut STGMEDIUM) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        let f = unsafe { &*pformatetc };
        if is_fmt(f, cf_descriptor(), TYMED_HGLOBAL.0 as u32)
            || is_fmt(f, cf_contents(), TYMED_ISTREAM.0 as u32)
        {
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
    fn EnumFormatEtc(&self, _dir: u32) -> Result<IEnumFORMATETC> {
        let mk = |cfmt: u16, tymed: u32| FORMATETC {
            cfFormat: cfmt,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed,
        };
        let fmts = [
            mk(cf_descriptor(), TYMED_HGLOBAL.0 as u32),
            mk(cf_contents(), TYMED_ISTREAM.0 as u32),
        ];
        unsafe { SHCreateStdEnumFmtEtc(&fmts) }
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
struct VirtSrc;

impl IDropSource_Impl for VirtSrc_Impl {
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

/// 원격 파일 1개를 가상 파일로 드래그한다(블로킹). 복사 드롭되면 true.
pub(crate) fn drag_out_remote(file: RemoteFile, sftp_id: SftpId, cmd_tx: Sender<Command>) -> bool {
    let data: IDataObject = VirtData { file, sftp_id, cmd_tx }.into();
    let src: IDropSource = VirtSrc.into();
    let mut effect = DROPEFFECT::default();
    let hr = unsafe { DoDragDrop(&data, &src, DROPEFFECT_COPY, &mut effect) };
    hr == DRAGDROP_S_DROP
}
