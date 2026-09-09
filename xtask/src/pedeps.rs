//! exe 가 **시작할 때 요구하는 DLL** 을 직접 읽어 낸다(배치 AO).
//!
//! ## 왜 필요한가
//!
//! v0.1.491 이 실행 자체가 안 되는 채로 나갔다. 내장 웹 브라우저를 붙이면서 exe 가
//! `WebView2Loader.dll` 을 요구하게 됐는데 설치본에 넣지 않았다. 개발 중에는 cargo 가
//! 그 DLL 을 빌드 폴더에 놓아 줘서 **아무 문제가 없었고**, 릴리스도 성공했으므로
//! 아무 경고가 없었다.
//!
//! "이 DLL 을 넣어라"를 손으로 적어 두면 다음에 다른 DLL 이 늘 때 또 같은 일이 난다.
//! 그래서 **exe 에게 직접 묻는다.**
//!
//! ## 어떻게 읽는가
//!
//! 윈도우 실행 파일(PE)에는 "내가 부를 함수는 이 DLL 들에 있다"는 표가 들어 있다.
//! 그 표를 따라가 이름만 모은다. 여기서는 **읽기만** 한다.
//!
//! ```text
//! MZ 머리 → 0x3C 자리에 PE 머리 위치 → 섹션 표 → 가져오기 표 → DLL 이름들
//! ```

/// 이 exe 가 요구하는 DLL 이름들(소문자).
pub fn imports(bytes: &[u8]) -> Result<Vec<String>, String> {
    let pe = u32::from_le_bytes(read4(bytes, 0x3c)?) as usize;
    if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err("PE 파일이 아니다".into());
    }
    let opt = pe + 24;
    let magic = u16::from_le_bytes([*at(bytes, opt)?, *at(bytes, opt + 1)?]);
    // 64비트는 표 목록이 16바이트 뒤에서 시작한다.
    let dirs = opt + if magic == 0x20b { 112 } else { 96 };
    // 목록의 **두 번째**가 가져오기 표다. 첫 번째는 내보내기 표라서, 잘못 읽으면
    // 자기 자신의 이름이 나온다(실제로 그렇게 한 번 틀렸다 — 시험이 잡았다).
    let import_rva = u32::from_le_bytes(read4(bytes, dirs + 8)?) as usize;
    // 목록의 **열넷째**는 "미루는 가져오기" 표다. 여기 적힌 DLL 은 시작할 때가 아니라
    // 처음 그 함수를 부를 때 열린다 — 그래서 없어도 프로그램은 **일단 뜬다.** 그러다
    // 어느 메뉴를 누르는 순간 죽는다. 시작할 때 터지는 것보다 찾기 어려우므로 함께 센다.
    let delay_rva = u32::from_le_bytes(read4(bytes, dirs + 13 * 8)?) as usize;
    let sections = section_table(bytes, pe)?;
    let mut out = Vec::new();
    // (표의 시작, 한 칸 크기, 이름이 놓인 자리)
    for (rva, step, at_name) in [(import_rva, 20, 12), (delay_rva, 32, 4)] {
        if rva == 0 {
            continue;
        }
        let mut off = to_offset(&sections, rva).ok_or("가져오기 표를 찾지 못했다")?;
        loop {
            // 이름이 0이면 표가 끝난 것이다.
            let name_rva = u32::from_le_bytes(read4(bytes, off + at_name)?) as usize;
            if name_rva == 0 {
                break;
            }
            if let Some(o) = to_offset(&sections, name_rva) {
                let name = cstr(bytes, o).to_ascii_lowercase();
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
            off += step;
        }
    }
    Ok(out)
}

/// 섹션 표 — (가상 주소, 크기, 파일 위치).
fn section_table(b: &[u8], pe: usize) -> Result<Vec<(usize, usize, usize)>, String> {
    let n = u16::from_le_bytes([*at(b, pe + 6)?, *at(b, pe + 7)?]) as usize;
    let opt_size = u16::from_le_bytes([*at(b, pe + 20)?, *at(b, pe + 21)?]) as usize;
    let start = pe + 24 + opt_size;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let s = start + i * 40;
        v.push((
            u32::from_le_bytes(read4(b, s + 12)?) as usize,
            u32::from_le_bytes(read4(b, s + 8)?) as usize,
            u32::from_le_bytes(read4(b, s + 20)?) as usize,
        ));
    }
    Ok(v)
}

/// 가상 주소를 파일 안 위치로 바꾼다.
fn to_offset(sections: &[(usize, usize, usize)], rva: usize) -> Option<usize> {
    sections
        .iter()
        .find(|(va, sz, _)| rva >= *va && rva < va + sz.max(&1))
        .map(|(va, _, raw)| raw + (rva - va))
}

fn at(b: &[u8], i: usize) -> Result<&u8, String> {
    b.get(i).ok_or_else(|| format!("파일이 잘렸다({i} 자리)"))
}

fn read4(b: &[u8], i: usize) -> Result<[u8; 4], String> {
    b.get(i..i + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| format!("파일이 잘렸다({i} 자리)"))
}

fn cstr(b: &[u8], i: usize) -> String {
    let end = b[i..].iter().position(|c| *c == 0).unwrap_or(0);
    String::from_utf8_lossy(&b[i..i + end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::imports;

    #[test]
    fn nonsense_is_refused_instead_of_panicking() {
        assert!(imports(&[]).is_err());
        assert!(imports(b"not a program at all").is_err());
        // MZ 로 시작하지만 그 뒤가 없는 것.
        assert!(imports(b"MZ").is_err());
    }

    #[test]
    fn our_own_program_asks_for_the_usual_windows_dlls() {
        // 이 시험이 이 파일이 생긴 이유다. 실제 exe 를 읽어 이름이 나오는지 본다.
        let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target")
            .join("debug")
            .join("nabi.exe");
        let Ok(bytes) = std::fs::read(&exe) else {
            // 아직 빌드하지 않았으면 건너뛴다 — 시험 때문에 빌드를 강제하지 않는다.
            return;
        };
        let names = imports(&bytes).expect("우리 exe 는 읽혀야 한다");
        assert!(names.iter().any(|n| n == "kernel32.dll"), "{names:?}");
        // 같은 이름이 두 표에 다 있으면 한 번만 나와야 한다 — 안 그러면 dist 가
        // 같은 DLL 을 두 번 복사하고, "왜 두 번 나오지" 를 다음 사람이 또 파야 한다.
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "이름이 겹쳐 나온다: {names:?}");
    }

    /// **C 런타임을 정적으로 넣기로 했다** — 그러니 재배포 DLL 을 부르면 안 된다.
    ///
    /// 2026-09-10에 사용자가 받은 것은 프로그램이 아니라 "VCRUNTIME140_1.dll 이 없어
    /// 코드 실행을 진행할 수 없습니다" 대화상자 하나였다. `.cargo/config.toml` 의
    /// `+crt-static` 가 풀리면 조용히 그 상태로 돌아간다 — 빌드도 시험도 다 통과한 채로.
    /// 그래서 **exe 에게 직접 묻는다.**
    #[test]
    fn 우리_exe_는_재배포_런타임을_부르지_않는다() {
        let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target")
            .join("debug")
            .join("nabi.exe");
        let Ok(bytes) = std::fs::read(&exe) else { return };
        let names = imports(&bytes).expect("우리 exe 는 읽혀야 한다");
        let bad: Vec<&String> = names
            .iter()
            .filter(|n| crate::dist::is_redistributable(n))
            .collect();
        // 여러 줄 안내는 raw 문자열로 둔다 — 백슬래시로 줄을 잇는 문자열은
        // `xtask raw-newline` 이 "힙독이 백슬래시를 먹은 자리"와 구별하지 못한다.
        const HINT: &str = r"
  .cargo/config.toml 의 +crt-static 이 풀렸거나,
  target/debug/nabi.exe 가 그 설정이 생기기 전에 만든 낡은 것이다(cargo build 로 다시 만들 것).";
        assert!(bad.is_empty(), "재배포 런타임을 부른다: {bad:?}{HINT}");
    }
}
