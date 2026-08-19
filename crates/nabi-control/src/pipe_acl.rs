//! 파이프 보안 기술자 — 현재 사용자 SID 전용 DACL(CP-5 G5).
//! 기본 보안 기술자 대신 명시적 "현재 사용자만 접근" DACL로 파이프를 만든다.

use std::ffi::c_void;

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: isize, access: u32, token: *mut isize) -> i32;
    fn GetTokenInformation(
        token: isize,
        class: u32,
        info: *mut c_void,
        len: u32,
        ret_len: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: *mut c_void, out: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        sddl: *const u16,
        revision: u32,
        sd: *mut *mut c_void,
        sd_size: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> isize;
    fn CloseHandle(h: isize) -> i32;
    fn LocalFree(p: *mut c_void) -> *mut c_void;
}

const TOKEN_QUERY: u32 = 0x0008;
const TOKEN_USER_CLASS: u32 = 1; // TOKEN_INFORMATION_CLASS::TokenUser
const SDDL_REVISION_1: u32 = 1;

/// Win32 SECURITY_ATTRIBUTES(레이아웃 고정).
#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}

/// 현재 프로세스 토큰의 사용자 SID 문자열("S-1-5-21-…").
pub fn current_user_sid() -> Option<String> {
    // SAFETY: 토큰 핸들과 크기 변수는 모두 스택 지역이고, 각 호출의 반환값을 확인한 뒤
    // 다음 단계로 넘어간다(실패 시 조기 반환).
    unsafe {
        let mut token = 0isize;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut need = 0u32;
        GetTokenInformation(token, TOKEN_USER_CLASS, std::ptr::null_mut(), 0, &mut need);
        let mut buf = vec![0u8; need.max(8) as usize];
        let ok = GetTokenInformation(
            token,
            TOKEN_USER_CLASS,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as u32,
            &mut need,
        );
        CloseHandle(token);
        if ok == 0 {
            return None;
        }
        // TOKEN_USER의 첫 필드가 SID 포인터.
        let sid = *(buf.as_ptr() as *const *mut c_void);
        let mut ws: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut ws) == 0 || ws.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *ws.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(ws, len));
        LocalFree(ws as *mut c_void);
        Some(s)
    }
}

/// 현재 사용자에게만 GA(모든 접근)를 허용하는 보호 DACL의 SECURITY_ATTRIBUTES 보유자.
/// Drop 시 보안 기술자(LocalAlloc)를 해제한다.
pub struct PipeSecurity {
    sa: SecurityAttributes,
}

impl PipeSecurity {
    /// 생성 실패 시 None(호출측은 기본 기술자로 폴백 + 경고 로그).
    pub fn current_user_only() -> Option<PipeSecurity> {
        let sid = current_user_sid()?;
        // P=보호(상속 차단), 현재 사용자만 GA.
        let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})").encode_utf16().chain([0]).collect();
        let mut sd: *mut c_void = std::ptr::null_mut();
        // SAFETY: sddl은 NUL로 끝나는 UTF-16이고, sd는 스택 지역 포인터의 주소다. 성공 여부와
        // sd null 여부를 아래에서 모두 확인한 뒤에만 사용한다.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || sd.is_null() {
            return None;
        }
        Some(PipeSecurity {
            sa: SecurityAttributes {
                length: std::mem::size_of::<SecurityAttributes>() as u32,
                descriptor: sd,
                inherit: 0,
            },
        })
    }

    /// tokio `create_with_security_attributes_raw`에 넘길 포인터.
    pub fn attrs_ptr(&mut self) -> *mut c_void {
        &mut self.sa as *mut SecurityAttributes as *mut c_void
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        if !self.sa.descriptor.is_null() {
            // SAFETY: descriptor는 ConvertStringSecurityDescriptorToSecurityDescriptorW가 준 것이고,
            // null이 아님을 위에서 확인했다. Drop은 한 번만 돌므로 이중 해제가 없다.
            unsafe { LocalFree(self.sa.descriptor) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_sid_resolves() {
        let sid = current_user_sid().expect("현재 사용자 SID");
        assert!(sid.starts_with("S-1-"), "{sid}");
    }

    #[test]
    fn security_descriptor_builds() {
        assert!(PipeSecurity::current_user_only().is_some());
    }
}
