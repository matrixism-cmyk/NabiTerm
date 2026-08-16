//! nabiPad 해시/체크섬 도구(HxD/CyberChef/DevTools 벤치마킹) — 텍스트 → 16진 다이제스트.
//! 암호 해시는 RustCrypto(sha1/sha2, 이미 SSH가 사용), 단순 해시는 자작. "해시" 서브메뉴.

use nabi_i18n::{tr, Lang};
use sha2::Digest;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha1_hex(t: &str) -> String {
    hex(sha1::Sha1::digest(t.as_bytes()).as_slice())
}
pub fn sha224_hex(t: &str) -> String {
    hex(sha2::Sha224::digest(t.as_bytes()).as_slice())
}
pub fn sha256_hex(t: &str) -> String {
    hex(sha2::Sha256::digest(t.as_bytes()).as_slice())
}
pub fn sha384_hex(t: &str) -> String {
    hex(sha2::Sha384::digest(t.as_bytes()).as_slice())
}
pub fn sha512_hex(t: &str) -> String {
    hex(sha2::Sha512::digest(t.as_bytes()).as_slice())
}

/// Adler-32 체크섬.
pub fn adler32(t: &str) -> String {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in t.as_bytes() {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    format!("{:08x}", (b << 16) | a)
}

/// FNV-1a 32비트.
pub fn fnv1a32(t: &str) -> String {
    let mut h = 0x811c_9dc5u32;
    for &x in t.as_bytes() {
        h ^= u32::from(x);
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// FNV-1a 64비트.
pub fn fnv1a64(t: &str) -> String {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in t.as_bytes() {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// djb2 해시(Bernstein).
pub fn djb2(t: &str) -> String {
    let mut h = 5381u32;
    for &x in t.as_bytes() {
        h = h.wrapping_mul(33).wrapping_add(u32::from(x));
    }
    format!("{h:08x}")
}

/// sdbm 해시.
pub fn sdbm(t: &str) -> String {
    let mut h = 0u32;
    for &x in t.as_bytes() {
        h = u32::from(x).wrapping_add(h << 6).wrapping_add(h << 16).wrapping_sub(h);
    }
    format!("{h:08x}")
}

/// CRC-32C(Castagnoli, 반사 다항식 0x82F63B78) — iSCSI/ext4/SSE4.2.
pub fn crc32c(t: &str) -> String {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in t.as_bytes() {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0x82F6_3B78 } else { crc >> 1 };
        }
    }
    format!("{:08x}", !crc)
}

/// CRC-16/CCITT-FALSE.
pub fn crc16(t: &str) -> String {
    let mut crc = 0xFFFFu16;
    for &x in t.as_bytes() {
        crc ^= u16::from(x) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    format!("{crc:04x}")
}

/// "해시" 서브메뉴 — 암호 해시/체크섬(논크립토) 하위 그룹으로 분류(2단계 계층).
pub fn hash_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    use crate::editmenugroups::pick;
    let mut picked = None;
    ui.menu_button(tr(lang, "editor.hashcrypto"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.md5", crate::editormd5::md5_hex), ("editor.sha1", sha1_hex),
            ("editor.sha224", sha224_hex), ("editor.sha256", sha256_hex),
            ("editor.sha384", sha384_hex), ("editor.sha512", sha512_hex),
        ]));
    });
    ui.menu_button(tr(lang, "editor.hashcksum"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.crc32h", crate::editordev2::crc32_lower), ("editor.crc32c", crc32c),
            ("editor.crc16", crc16), ("editor.adler32", adler32),
            ("editor.fnv32", fnv1a32), ("editor.fnv64", fnv1a64),
            ("editor.djb2", djb2), ("editor.sdbm", sdbm),
        ]));
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypto_vectors() {
        // 표준 "abc" 벡터.
        assert_eq!(sha1_hex("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(sha224_hex("abc"), "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7");
        assert_eq!(sha256_hex("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(sha512_hex("abc").starts_with("ddaf35a193617aba"));
        assert!(sha384_hex("abc").starts_with("cb00753f45a35e8b"));
    }

    #[test]
    fn simple_hashes() {
        assert_eq!(fnv1a32("a"), "e40c292c"); // 알려진 FNV-1a 값.
        assert_eq!(adler32("Wikipedia"), "11e60398");
        assert_eq!(crc16("123456789"), "29b1"); // CRC-16/CCITT-FALSE 검사값.
        assert_eq!(crc32c("123456789"), "e3069283"); // CRC-32C 표준 검사값.
        assert_eq!(djb2("").len(), 8); // 형식만(빈 입력).
        assert_eq!(sdbm("").len(), 8);
    }
}
