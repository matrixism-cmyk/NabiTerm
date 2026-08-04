//! 테스트 서버의 OpenSSH 확장 구현 — posix-rename·fsync·statvfs·limits.
//!
//! 이게 있어야 클라이언트의 확장 경로(원자적 교체·청크 협상·여유공간 확인)를
//! 실서버 없이도 검증할 수 있다. 값은 검증에 필요한 최소한만 채운다.

use crate::sftp_server::{ok_status, Sftp};
use russh_sftp::protocol::{ExtendedReply, Packet, StatusCode};
use std::collections::HashMap;

/// 테스트 서버가 광고하는 확장(VERSION 응답).
pub(crate) fn advertised() -> HashMap<String, String> {
    [
        ("posix-rename@openssh.com", "1"),
        ("fsync@openssh.com", "1"),
        ("statvfs@openssh.com", "2"),
        ("limits@openssh.com", "1"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// 테스트 서버가 알려주는 한도 — 클라이언트가 이 값으로 청크를 정하는지 보려고 일부러 작게 둔다.
pub(crate) const TEST_READ_LEN: u64 = 8 * 1024;
pub(crate) const TEST_WRITE_LEN: u64 = 8 * 1024;
/// 서버가 한 응답에 실제로 담아 주는 최대 바이트(테스트가 요청 효율을 계산할 때 쓴다).
pub(crate) use crate::sftp_server::SHORT_READ_CAP as SHORT_READ_CAP_FOR_TEST;
/// statvfs가 보고할 여유 블록 수(× fragment_size).
pub(crate) const TEST_FREE_BLOCKS: u64 = 1024;
pub(crate) const TEST_FRAGMENT: u64 = 4096;

/// 길이(u32 BE) 접두 문자열을 앞에서부터 두 개 읽는다.
fn two_strings(data: &[u8]) -> Option<(String, String)> {
    let mut at = 0usize;
    let mut next = || {
        let len = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as usize;
        let s = String::from_utf8(data.get(at + 4..at + 4 + len)?.to_vec()).ok()?;
        at += 4 + len;
        Some(s)
    };
    Some((next()?, next()?))
}

fn u64s(values: &[u64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

/// 확장 요청 처리. 모르는 요청은 OpUnsupported(규격).
pub(crate) fn handle(s: &mut Sftp, id: u32, request: &str, data: &[u8]) -> Result<Packet, StatusCode> {
    match request {
        "posix-rename@openssh.com" => {
            let (from, to) = two_strings(data).ok_or(StatusCode::BadMessage)?;
            let buf = s.files.remove(&from).ok_or(StatusCode::NoSuchFile)?;
            s.files.insert(to, buf); // 대상이 있어도 그대로 덮어쓴다(원자적 교체).
            Ok(Packet::Status(ok_status(id)))
        }
        "fsync@openssh.com" => Ok(Packet::Status(ok_status(id))),
        "limits@openssh.com" => {
            let d = u64s(&[64 * 1024, TEST_READ_LEN, TEST_WRITE_LEN, 64]);
            Ok(Packet::ExtendedReply(ExtendedReply { id, data: d }))
        }
        "statvfs@openssh.com" => {
            // block_size, fragment_size, blocks, free, avail, inodes, ifree, iavail, fsid, flags, namemax
            let d = u64s(&[
                TEST_FRAGMENT,
                TEST_FRAGMENT,
                TEST_FREE_BLOCKS * 4,
                TEST_FREE_BLOCKS,
                TEST_FREE_BLOCKS,
                1000,
                900,
                900,
                1,
                0,
                255,
            ]);
            Ok(Packet::ExtendedReply(ExtendedReply { id, data: d }))
        }
        _ => Err(StatusCode::OpUnsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::two_strings;

    #[test]
    fn parses_two_length_prefixed_strings() {
        let data = [0, 0, 0, 2, b'a', b'b', 0, 0, 0, 1, b'c'];
        assert_eq!(two_strings(&data), Some(("ab".into(), "c".into())));
        assert_eq!(two_strings(&[0, 0, 0, 9]), None, "잘린 페이로드는 거부");
    }
}
