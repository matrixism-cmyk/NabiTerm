//! 빌드 스크립트: 나비 아이콘(ICO)을 절차 생성해 exe 리소스로 임베드(탐색기 아이콘).
//! 실패해도 빌드는 계속한다(아이콘만 빠짐 — windres 부재 등).

/// appicon.rs와 동일한 나비 64×64 RGBA(빌드 시점 복제 — 크레이트 공유 불가).
fn butterfly_rgba(s: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; s * s * 4];
    let f = s as f32 / 64.0; // 64 기준 좌표 스케일.
    let wings: [(f32, f32, f32, f32); 4] = [
        (22.0, 24.0, 15.0, 12.5),
        (42.0, 24.0, 15.0, 12.5),
        (24.0, 43.0, 11.0, 9.5),
        (40.0, 43.0, 11.0, 9.5),
    ];
    for y in 0..s {
        for x in 0..s {
            let (fx, fy) = (x as f32 / f + 0.5, y as f32 / f + 0.5);
            let mut a = 0.0f32;
            for (cx, cy, rx, ry) in wings {
                let d = ((fx - cx) / rx).powi(2) + ((fy - cy) / ry).powi(2);
                if d < 1.0 {
                    a = a.max(1.0 - d * d);
                }
            }
            let i = (y * s + x) * 4;
            if a > 0.02 {
                let t = fx / 64.0;
                rgba[i] = (64.0 + 86.0 * t) as u8;
                rgba[i + 1] = (180.0 - 70.0 * t) as u8;
                rgba[i + 2] = (230.0 + 5.0 * t) as u8;
                rgba[i + 3] = (a.min(1.0) * 255.0) as u8;
            }
            let bd = ((fx - 32.0) / 2.6).powi(2) + ((fy - 33.0) / 16.0).powi(2);
            if bd < 1.0 {
                rgba[i] = 30;
                rgba[i + 1] = 36;
                rgba[i + 2] = 48;
                rgba[i + 3] = 255;
            }
        }
    }
    rgba
}

/// 한 크기의 BMP 페이로드(BITMAPINFOHEADER + 32bpp bottom-up + 빈 AND 마스크).
fn ico_image(size: usize) -> Vec<u8> {
    let rgba = butterfly_rgba(size);
    let mask_row = size.div_ceil(32) * 4; // AND 마스크 행(32비트 패딩).
    let mut img = Vec::with_capacity(40 + size * size * 4 + mask_row * size);
    img.extend_from_slice(&40u32.to_le_bytes());
    img.extend_from_slice(&(size as i32).to_le_bytes());
    img.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // 높이=2배(XOR+AND).
    img.extend_from_slice(&[1, 0, 32, 0]);
    img.extend_from_slice(&[0u8; 24]); // 압축/크기/해상도 등 0.
    for y in (0..size).rev() {
        for x in 0..size {
            let i = (y * size + x) * 4;
            img.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    img.extend(std::iter::repeat_n(0u8, mask_row * size)); // AND 마스크=0(알파 사용).
    img
}

/// 다중 크기 ICO — 탐색기 목록(16/24/32)·바로가기(48)·타일(256)까지 또렷하게.
fn encode_ico(sizes: &[usize]) -> Vec<u8> {
    let images: Vec<(usize, Vec<u8>)> = sizes.iter().map(|&s| (s, ico_image(s))).collect();
    let mut out = vec![0, 0, 1, 0, sizes.len() as u8, 0]; // ICONDIR.
    let mut off = 6 + 16 * sizes.len();
    for (s, img) in &images {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&(off as u32).to_le_bytes());
        off += img.len();
    }
    for (_, img) in &images {
        out.extend_from_slice(img);
    }
    out
}

/// 웹 화면을 여는 데 필요한 `WebView2Loader.dll` 을 **빌드한 exe 옆에 갖다 놓는다.**
///
/// ## 왜 빌드 때 하는가
///
/// 이 DLL 이 없으면 exe 가 아예 뜨지 않는다. 창 하나 없이 "WebView2Loader.dll was not
/// found" 라는 윈도우 오류 상자만 뜬다 — 프로그램이 고장 난 것처럼 보인다.
///
/// 설치본은 `xtask dist` 가 챙긴다. 하지만 `cargo run` 이나 `target/debug/nabi.exe` 를
/// 그냥 실행하는 개발 중에는 아무도 챙기지 않았다. 실제로 두 번 당했다 — 한 번은 릴리스
/// (v0.1.491), 한 번은 개발 중(2026-08-29, 오류 상자가 사용자 화면에까지 떴다).
///
/// 챙기는 자리를 **빌드로 옮기면** 두 경우가 한 번에 해결된다. 빌드한 결과 옆에 늘 있다.
///
/// 실패해도 빌드는 계속한다 — 웹 기능만 못 쓰고 나머지는 멀쩡하다.
fn copy_webview_loader() {
    // OUT_DIR = target/<프로파일>/build/<크레이트>-<해시>/out → 네 단계 위가 exe 자리.
    let Ok(out_dir) = std::env::var("OUT_DIR") else { return };
    let out = std::path::PathBuf::from(&out_dir);
    let Some(profile_dir) = out.ancestors().nth(3) else { return };
    let dest = profile_dir.join("WebView2Loader.dll");
    if dest.exists() {
        return; // 이미 있으면 그대로 둔다 — 빌드마다 덮어쓸 이유가 없다.
    }
    // 같은 build 폴더 아래 webview2-com-sys 가 풀어 놓은 x64 판을 찾는다.
    let Some(build_dir) = out.ancestors().nth(2) else { return };
    let Ok(rd) = std::fs::read_dir(build_dir) else { return };
    for e in rd.flatten() {
        if !e.file_name().to_string_lossy().starts_with("webview2-com-sys-") {
            continue;
        }
        let src = e.path().join("out").join("x64").join("WebView2Loader.dll");
        if src.exists() && std::fs::copy(&src, &dest).is_ok() {
            return;
        }
    }
    println!("cargo:warning=WebView2Loader.dll 을 찾지 못했다 — 내장 웹 브라우저가 뜨지 않는다.");
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("nabi.ico");
    let ico = encode_ico(&[16, 24, 32, 48, 64, 128, 256]);
    if std::fs::write(&out, ico).is_err() {
        return;
    }
    let mut res = winres::WindowsResource::new();
    res.set_icon(out.to_str().unwrap());
    // 탐색기 속성/작업관리자에 표시될 제품 정보(표시명은 nabiTerm, 파일명은 nabi 유지).
    res.set("ProductName", "nabiTerm");
    res.set("FileDescription", "nabiTerm \u{2014} terminal multiplexer & SSH client");
    res.set("LegalCopyright", "\u{00a9} 2026 aeo");
    if let Err(e) = res.compile() {
        println!("cargo:warning=아이콘 임베드 생략: {e}");
    }
    copy_webview_loader();
}
