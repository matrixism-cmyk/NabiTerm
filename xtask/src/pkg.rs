//! `xtask pkg` — **패키지 저장소 매니페스트를 산출물에서 만들어 준다.**
//!
//! ## 왜 필요한가
//!
//! 지금 배포 통로는 GitHub 릴리스 하나뿐이다. 받으려면 저장소를 찾아 들어와야 하고,
//! 찾아 들어오려면 이미 알고 있어야 한다. winget·Scoop에 올리면 `winget install nabiTerm`
//! 한 줄로 닿는다.
//!
//! 덤이 하나 더 있다. 우리는 사용 기록을 보내지 않으므로(폐쇄망이 정체성이다) 우리가
//! 셀 수 있는 숫자가 없다. 패키지 저장소는 **남이 세어 준다** — 원칙을 지키면서 얻는
//! 유일한 지표다.
//!
//! ## 왜 손으로 적지 않는가
//!
//! 매니페스트에는 버전·URL·SHA-256이 들어간다. 셋 다 릴리스마다 바뀌고, 사람이 적으면
//! 언젠가 어긋난다. 어긋난 해시는 **설치 실패**로 나타나는데 그때는 이미 배포된 뒤다.
//! 저장소 이름을 문서에 적었다가 일곱 판을 잃은 일과 같은 결이라(`releasetarget.rs`),
//! 여기서도 **산출물이 유일한 출처**다 — 해시는 파일에서 직접 계산한다.

use std::path::Path;
use std::process::ExitCode;

/// winget 패키지 식별자. 게시자와 이름은 저장소 소유자·상표와 맞춘다.
const WINGET_ID: &str = "Nabisori.nabiTerm";
const PUBLISHER: &str = "Nabisori";
const HOMEPAGE: &str = "https://nabisori.kr/nabiterm.php";

/// 매니페스트를 만들어 `dist/pkg/`에 떨군다.
pub fn run() -> ExitCode {
    let root = std::env::current_dir().unwrap_or_default();
    let setup = root.join("dist").join("nabiTerm-setup.exe");
    if !setup.is_file() {
        eprintln!("먼저 `cargo run -p xtask -- dist` 로 설치 파일을 만들어야 한다: {}", setup.display());
        return ExitCode::FAILURE;
    }
    let Some(version) = workspace_version(&root) else {
        eprintln!("Cargo.toml 에서 버전을 읽지 못했다");
        return ExitCode::FAILURE;
    };
    let sha = match sha256_of(&setup) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("해시를 읽지 못했다: {e}");
            return ExitCode::FAILURE;
        }
    };
    let repo = crate::releasetarget::app_repo();
    let url = asset_url(&repo, &version);

    let out = root.join("dist").join("pkg");
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    // winget 세 파일은 **제출 폴더 구조 그대로** 떨군다 — 그대로 복사해 PR 하면 된다.
    // (`manifests/n/Nabisori/nabiTerm/<버전>/`)
    let wdir = out.join("winget").join(&version);
    if let Err(e) = std::fs::create_dir_all(&wdir) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    let mut files: Vec<(std::path::PathBuf, String)> = winget_manifests(&version, &url, &sha)
        .into_iter()
        .map(|(n, b)| (wdir.join(n), b))
        .collect();
    files.push((out.join("nabiTerm.scoop.json"), scoop_manifest(&version, &url, &sha, &repo)));
    for (p, body) in files {
        if let Err(e) = std::fs::write(&p, body) {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
        println!("생성: {}", p.display());
    }
    println!("winget 제출 경로: manifests/n/{PUBLISHER}/nabiTerm/{version}/");
    println!("버전 v{version} · SHA256 {sha}");
    ExitCode::SUCCESS
}

/// 릴리스 자산 주소. 자산 이름은 판마다 같아야 이 형태가 성립한다(우리는 그렇다).
pub fn asset_url(repo: &str, version: &str) -> String {
    format!("https://github.com/{repo}/releases/download/v{version}/nabiTerm-setup.exe")
}

/// winget 매니페스트 세 벌(version · installer · defaultLocale).
///
/// **`winget-pkgs` 는 단일 매니페스트를 받지 않는다.** 처음에는 singleton 스키마로
/// 하나만 만들었는데, 실제 저장소를 열어 보니 모든 패키지가 세 파일로 되어 있었다
/// (2026-08-27에 확인). 제출할 수 없는 형식을 만들어 두는 것은 안 만든 것보다 나쁘다 —
/// 다 됐다고 착각하게 만들기 때문이다.
///
/// 설치 방식은 Inno Setup(`inno`)이고 무인 스위치를 우리가 직접 넘긴다.
pub fn winget_manifests(version: &str, url: &str, sha: &str) -> Vec<(String, String)> {
    let sha_up = sha.to_uppercase();
    let head = |kind: &str| {
        [
            "# `cargo run -p xtask -- pkg` 가 만든다 — 손으로 고치지 말 것.".to_string(),
            format!("# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.1.6.0.schema.json"),
            String::new(),
            format!("PackageIdentifier: {WINGET_ID}"),
            format!("PackageVersion: {version}"),
        ]
        .join("
")
    };

    let version_yaml = [
        head("version"),
        "DefaultLocale: en-US".into(),
        "ManifestType: version".into(),
        "ManifestVersion: 1.6.0".into(),
        String::new(),
    ]
    .join("
");

    let installer_yaml = [
        head("installer"),
        "InstallerType: inno".into(),
        "Scope: user".into(),
        "InstallerSwitches:".into(),
        "  Silent: /VERYSILENT /NOLAUNCH".into(),
        "  SilentWithProgress: /SILENT /NOLAUNCH".into(),
        "Installers:".into(),
        "- Architecture: x64".into(),
        format!("  InstallerUrl: {url}"),
        format!("  InstallerSha256: {sha_up}"),
        "ManifestType: installer".into(),
        "ManifestVersion: 1.6.0".into(),
        String::new(),
    ]
    .join("
");

    let locale_yaml = [
        head("defaultLocale"),
        "PackageLocale: en-US".into(),
        format!("Publisher: {PUBLISHER}"),
        "PublisherUrl: https://nabisori.kr".into(),
        "PublisherSupportUrl: https://github.com/matrixism-cmyk/NabiTerm/issues".into(),
        "PackageName: nabiTerm".into(),
        format!("PackageUrl: {HOMEPAGE}"),
        "License: Apache-2.0".into(),
        "LicenseUrl: https://github.com/matrixism-cmyk/NabiTerm/blob/main/LICENSE".into(),
        "ShortDescription: Windows terminal multiplexer with SSH/SFTP client and editor".into(),
        "Description: |-".into(),
        "  A native Windows terminal, professional SFTP client and code editor in one window.".into(),
        "  Korean by default, free, Apache-2.0. Includes a local control plane and MCP server".into(),
        "  so AI agents running in a pane can drive the terminal and move files over SFTP.".into(),
        "Moniker: nabiterm".into(),
        "Tags:".into(),
        "- terminal".into(),
        "- ssh".into(),
        "- sftp".into(),
        "- editor".into(),
        "- korean".into(),
        "- rust".into(),
        "ManifestType: defaultLocale".into(),
        "ManifestVersion: 1.6.0".into(),
        String::new(),
    ]
    .join("
");

    vec![
        (format!("{WINGET_ID}.yaml"), version_yaml),
        (format!("{WINGET_ID}.installer.yaml"), installer_yaml),
        (format!("{WINGET_ID}.locale.en-US.yaml"), locale_yaml),
    ]
}
/// Scoop 매니페스트. `checkver`·`autoupdate`가 있으면 저장소가 **스스로 새 판을 따라온다**
/// — 우리가 매번 올리러 가지 않아도 되므로 통로가 살아 있게 유지된다.
pub fn scoop_manifest(version: &str, url: &str, sha: &str, repo: &str) -> String {
    [
        "{",
        &format!("  \"version\": \"{version}\","),
        "  \"description\": \"Windows terminal multiplexer with SSH/SFTP client and editor\",",
        &format!("  \"homepage\": \"{HOMEPAGE}\","),
        "  \"license\": \"Apache-2.0\",",
        "  \"architecture\": {",
        "    \"64bit\": {",
        &format!("      \"url\": \"{url}\","),
        &format!("      \"hash\": \"{sha}\""),
        "    }",
        "  },",
        "  \"installer\": {",
        "    \"script\": [",
        "      \"Start-Process -Wait -FilePath \\\"$dir\\\\nabiTerm-setup.exe\\\" -ArgumentList '/VERYSILENT','/NOLAUNCH'\"",
        "    ]",
        "  },",
        "  \"checkver\": {",
        &format!("    \"github\": \"https://github.com/{repo}\""),
        "  },",
        "  \"autoupdate\": {",
        "    \"architecture\": {",
        "      \"64bit\": {",
        &format!("        \"url\": \"https://github.com/{repo}/releases/download/v$version/nabiTerm-setup.exe\""),
        "      }",
        "    }",
        "  }",
        "}",
        "",
    ]
    .join("\n")
}

/// 워크스페이스 버전(루트 `Cargo.toml`의 첫 `version = "…"`).
fn workspace_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    parse_version(&text)
}

/// `version = "0.1.2"` 에서 값만 뽑는다.
pub fn parse_version(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("version") {
            let rest = rest.trim_start().strip_prefix('=')?.trim();
            let v = rest.trim_matches('"');
            if !v.is_empty() && v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// 파일의 SHA-256(소문자 16진). 외부 도구 없이 스스로 센다.
fn sha256_of(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finish())
}

/// 아주 작은 SHA-256 — xtask에 의존성을 더하지 않으려고 직접 둔다.
struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    len: usize,
    total: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    fn new() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0; 64],
            len: 0,
            total: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total += data.len() as u64;
        while !data.is_empty() {
            let n = (64 - self.len).min(data.len());
            self.buf[self.len..self.len + n].copy_from_slice(&data[..n]);
            self.len += n;
            data = &data[n..];
            if self.len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.len = 0;
            }
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[i * 4], block[i * 4 + 1], block[i * 4 + 2], block[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = self.state;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for (s, add) in self.state.iter_mut().zip(v) {
            *s = s.wrapping_add(add);
        }
    }

    fn finish(mut self) -> String {
        let bits = self.total * 8;
        self.update(&[0x80]);
        while self.len != 56 {
            self.update(&[0]);
        }
        let b = bits.to_be_bytes();
        self.update(&b);
        let mut out = String::with_capacity(64);
        for word in self.state {
            out.push_str(&format!("{word:08x}"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 우리 해시 구현이 표준 값과 같은가 — 틀리면 **설치가 실패한다.**
    #[test]
    fn the_hash_matches_the_known_answers() {
        let mut h = Sha256::new();
        h.update(b"");
        assert_eq!(h.finish(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        let mut h = Sha256::new();
        h.update(b"abc");
        assert_eq!(h.finish(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

        // 블록 경계를 넘는 길이(64바이트 이상)도 맞아야 한다.
        let mut h = Sha256::new();
        h.update(&[b'a'; 1000]);
        assert_eq!(h.finish(), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
    }

    /// 조각내어 넣어도 결과가 같아야 한다(파일을 1MB씩 읽어 넣는다).
    #[test]
    fn feeding_in_pieces_gives_the_same_answer() {
        let data: Vec<u8> = (0..500u32).map(|i| (i % 251) as u8).collect();
        let mut one = Sha256::new();
        one.update(&data);
        let mut many = Sha256::new();
        for chunk in data.chunks(7) {
            many.update(chunk);
        }
        assert_eq!(one.finish(), many.finish());
    }

    #[test]
    fn the_version_comes_out_of_the_toml() {
        let toml = "[workspace.package]\nedition = \"2021\"\nversion = \"0.1.481\"\n";
        assert_eq!(parse_version(toml).as_deref(), Some("0.1.481"));
        assert_eq!(parse_version("name = \"x\"\n"), None);
    }

    /// 매니페스트에 **버전·주소·해시가 실제로 박혀야** 한다 — 셋 중 하나라도 빠지면
    /// 설치가 실패하거나 옛 판이 깔린다.
    #[test]
    fn the_manifests_carry_version_url_and_hash() {
        let url = asset_url("owner/Repo", "1.2.3");
        assert!(url.contains("/releases/download/v1.2.3/nabiTerm-setup.exe"), "{url}");

        let sha = "abc123";
        // winget-pkgs 는 세 파일을 받는다 — 하나라도 빠지면 PR 이 거부된다.
        let w = winget_manifests("1.2.3", &url, sha);
        let names: Vec<&str> = w.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names.len(), 3, "{names:?}");
        assert!(names.iter().any(|n| n.ends_with(".installer.yaml")), "{names:?}");
        assert!(names.iter().any(|n| n.ends_with(".locale.en-US.yaml")), "{names:?}");
        let all: String = w.iter().map(|(_, b)| b.as_str()).collect::<Vec<_>>().join("
");
        assert!(all.contains("PackageVersion: 1.2.3"));
        assert!(all.contains(&url));
        assert!(all.contains("ABC123"), "winget 해시는 대문자다");
        assert!(all.contains("InstallerType: inno"));
        assert!(all.contains("/VERYSILENT /NOLAUNCH"), "무인 스위치가 빠졌다");
        // 세 파일 모두 자기 종류를 밝혀야 한다.
        for kind in ["ManifestType: version", "ManifestType: installer", "ManifestType: defaultLocale"] {
            assert!(all.contains(kind), "{kind} 가 없다");
        }

        let s = scoop_manifest("1.2.3", &url, sha, "owner/Repo");
        assert!(s.contains("\"version\": \"1.2.3\""), "{s}");
        assert!(s.contains(&format!("\"hash\": \"{sha}\"")));
        assert!(s.contains("\"autoupdate\""), "자동 갱신이 없으면 통로가 곧 낡는다");
    }

    /// Scoop 매니페스트는 **JSON으로 읽혀야 한다** — 따옴표 하나 어긋나면 저장소가 거부한다.
    #[test]
    fn the_scoop_manifest_is_balanced_json() {
        let s = scoop_manifest("1.0.0", "https://x/y.exe", "deadbeef", "o/r");
        let (mut depth, mut quotes) = (0i32, 0usize);
        let mut prev = ' ';
        for c in s.chars() {
            match c {
                '"' if prev != '\\' => quotes += 1,
                '{' if quotes % 2 == 0 => depth += 1,
                '}' if quotes % 2 == 0 => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "닫는 괄호가 먼저 나왔다");
            prev = c;
        }
        assert_eq!(depth, 0, "괄호가 맞지 않는다:\n{s}");
        assert_eq!(quotes % 2, 0, "따옴표가 홀수다");
    }
}
