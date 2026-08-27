//! editornum 단위 테스트(editornum.rs 라인 한도 유지를 위해 분리).

use crate::editornum::*;

#[test]
fn bases() {
    assert_eq!(dec_to_hex("255\n16"), "ff\n10");
    assert_eq!(hex_to_dec("ff\n0x10"), "255\n16");
    assert_eq!(dec_to_bin("5"), "101");
    assert_eq!(bin_to_dec("101"), "5");
    assert_eq!(dec_to_oct("64"), "100");
    assert_eq!(oct_to_dec("100"), "64");
    assert_eq!(hex_to_bin("f"), "1111");
    assert_eq!(bin_to_hex("11111111"), "ff");
    assert_eq!(dec_to_hex("nope"), "nope"); // 실패 줄 보존.
    assert_eq!(dec_to_base36("1295"), "zz");
    assert_eq!(base36_to_dec("zz"), "1295");
    assert_eq!(dec_to_base62("62"), "10");
    assert_eq!(base62_to_dec("10"), "62");
    assert_eq!(dec_to_base62("61"), "z");
    assert_eq!(percent_to_decimal("50%"), "0.5");
    assert_eq!(decimal_to_percent("0.5"), "50%");
    assert_eq!(decimal_to_percent("0.1"), "10%"); // 부동소수 꼬리 정리.
    assert_eq!(sec_to_hms("3661"), "1:01:01");
    assert_eq!(sec_to_hms("61"), "1:01");
    assert_eq!(hms_to_sec("1:01:01"), "3661");
    assert_eq!(hms_to_sec("90"), "90");
    assert_eq!(celsius_to_fahrenheit("100"), "212");
    assert_eq!(fahrenheit_to_celsius("212"), "100");
    assert_eq!(to_ordinal("1\n2\n3\n11\n21\n113"), "1st\n2nd\n3rd\n11th\n21st\n113th");
    assert_eq!(bytes_to_bits("1024"), "8192");
    assert_eq!(bits_to_bytes("8192"), "1024");
}

#[test]
fn roman_and_human() {
    assert_eq!(dec_to_roman("2024"), "MMXXIV");
    assert_eq!(roman_to_dec("MMXXIV"), "2024");
    assert_eq!(roman_to_dec("IIII"), "IIII"); // 잘못된 표기→원문.
    assert_eq!(bytes_human("1536"), "1.50 KB");
    assert_eq!(bytes_human("1048576"), "1.00 MB");
    assert_eq!(human_bytes("1.5 KB"), "1536");
}

#[test]
fn float_hex_roundtrip() {
    assert_eq!(float_to_hex("1.0"), "3f800000");
    assert_eq!(float_to_hex("1.5"), "3fc00000");
    assert_eq!(hex_to_float("3f800000"), "1");
    assert_eq!(hex_to_float("0x3fc00000"), "1.5");
    assert_eq!(float_to_hex("nope"), "nope"); // 실패 줄 보존.
}

/// **만든 것을 되읽을 수 있는가** — `bytes_human` 이 낸 것을 `human_bytes` 가 받아야 한다.
///
/// 예전에는 `human` 이 EB 를 만드는데 `parse_human` 이 EB 를 몰랐다. 그런데 못 읽었다고
/// 알려 주지도 않는다 — `per_line` 이 실패한 줄을 원문 그대로 두기 때문에, 사용자는
/// "되돌리기"를 눌렀는데 아무 일도 안 일어난 것처럼 보게 된다. 조용한 실패가 가장 나쁘다.
#[test]
fn every_unit_we_emit_can_be_read_back() {
    for n in [0u64, 1, 1023, 1024, 1536, 1_048_576, 1_073_741_824,
              1_099_511_627_776, 1_125_899_906_842_624,
              1_152_921_504_606_846_976, u64::MAX] {
        let h = bytes_human(&n.to_string());
        let back = human_bytes(&h);
        assert_ne!(back, h, "{n} → {h} 를 되읽지 못했다(원문이 그대로 남았다)");
        let got: u64 = back.parse().unwrap_or_else(|_| panic!("{n} → {h} → {back} 가 숫자가 아니다"));
        // 사람 단위는 반올림하므로 값이 정확히 같을 수는 없다. 1% 안이면 같은 크기로 본다.
        let diff = got.abs_diff(n) as f64;
        assert!(diff <= (n as f64 * 0.01).max(1.0), "{n} → {h} → {got} 는 너무 멀다");
    }
}
