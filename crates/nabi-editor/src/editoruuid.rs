//! 에디터 삽입용 개발자 도구 — UUID v4·현재 날짜시각·임의 비밀번호. 커서 위치에 삽입(editorctx).

/// 임의 UUID v4 문자열(소문자, 8-4-4-4-12)을 생성한다.
pub fn gen_uuid_v4() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40; // 버전 4.
    b[8] = (b[8] & 0x3f) | 0x80; // 변형(variant) 10xx.
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// 현재 로컬 날짜·시각(ISO 비슷한 `YYYY-MM-DD HH:MM:SS`).
pub fn now_datetime() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 현재 Unix 타임스탬프(초). 개발·테스트 데이터에 epoch를 빠르게 넣을 때.
pub fn now_unix() -> String {
    chrono::Local::now().timestamp().to_string()
}

/// 표준 로렘 입숨 자리표시 문단(레이아웃/디자인 채움용).
pub fn lorem() -> String {
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".to_string()
}

/// 임의 강한 비밀번호(영숫자 + 기호, 헷갈리는 0/O/1/l 제외). len자.
pub fn gen_password(len: usize) -> String {
    use rand::Rng;
    const A: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789!@#$%^&*-_=+";
    let mut r = rand::rngs::OsRng;
    (0..len).map(|_| A[r.gen_range(0..A.len())] as char).collect()
}

#[cfg(test)]
mod tests {
    use super::gen_uuid_v4;

    #[test]
    fn uuid_v4_format() {
        let u = gen_uuid_v4();
        assert_eq!(u.len(), 36);
        let parts: Vec<&str> = u.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert_eq!(&u[14..15], "4"); // 버전 nibble.
        assert!("89ab".contains(&u[19..20])); // variant nibble.
        assert!(u.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
        assert_ne!(gen_uuid_v4(), u); // 매번 다름.
    }

    #[test]
    fn datetime_and_password() {
        let d = super::now_datetime();
        assert_eq!(d.len(), 19); // YYYY-MM-DD HH:MM:SS
        assert_eq!(&d[4..5], "-");
        let p = super::gen_password(16);
        assert_eq!(p.chars().count(), 16);
        assert!(!p.contains(['0', 'O', '1', 'l'])); // 헷갈리는 문자 제외.
        assert_ne!(super::gen_password(16), p); // 매번 다름.
        assert!(super::lorem().starts_with("Lorem ipsum")); // 표준 자리표시 문단.
        let u = super::now_unix();
        assert!(u.len() >= 10 && u.chars().all(|c| c.is_ascii_digit())); // epoch 초(전부 숫자).
    }
}
