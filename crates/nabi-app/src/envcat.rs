//! 환경/도구 **카탈로그** — 이 PC에 무엇을 갖출 수 있는지의 단일 출처.
//!
//! CLI에 익숙하지 않은 사용자에게 가장 어려운 것은 명령이 아니라 **선택지를 아는 것**이다.
//! `wsl --install -d <이름>`을 알려 줘도 어떤 이름을 쓸 수 있는지 모르면 소용이 없다.
//! 그래서 목록을 우리가 대신 읽어다 보여 준다.
//!
//! ## 왜 통로가 둘인가
//!
//! winget이 있으면 winget이 제일 깔끔하다. 그런데 **Windows Server에는 winget이 없다**
//! (2026-08-25에 이 개발 PC에서 확인 — Server 2022, 앱 설치 관리자 없음). winget만 쓰면
//! 서버 사용자에게는 이 화면 전체가 죽는다. 그래서 도구마다 직접 내려받는 길을 함께 둔다.
//!
//! ## 왜 스크립트가 진행 단계를 직접 말하는가
//!
//! 설치 진행바가 실제 진행과 따로 놀면 사용자는 기다리지 않고 창을 닫는다(직접 받은 지적).
//! 우리가 시간을 추측하는 대신 스크립트가 `@@STEP i/n 메시지`를 뱉고 UI는 그것만 읽는다.

/// 도구가 속한 묶음 — 화면의 소제목이 된다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Group {
    /// 셸 자체(PowerShell 7 등).
    Shell,
    /// 개발 도구(gh, ripgrep …).
    DevTool,
    /// 패키지 관리자 자체(winget). 이게 있어야 나머지가 쉬워지므로 맨 위에 둔다.
    Pkg,
}

/// 카탈로그 한 줄.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tool {
    pub id: &'static str,
    pub name: &'static str,
    /// PATH에서 이 이름을 찾으면 설치된 것으로 본다.
    pub probe: &'static str,
    pub group: Group,
    /// winget 패키지 id(있으면 우선 사용).
    pub winget: Option<&'static str>,
    /// winget이 없을 때 쓰는 PowerShell 설치 스크립트. `@@STEP`을 뱉어야 한다.
    pub fallback: Option<&'static str>,
    /// 제거 스크립트(winget이 있으면 winget으로 제거한다).
    pub remove: Option<&'static str>,
    /// 같은 도구의 **Microsoft Store판** 패키지 이름(있으면).
    ///
    /// 스토어판이 깔려 있으면 `WindowsApps\`의 앱 실행 별칭이 PATH를 먼저 차지한다.
    /// 그 계정에 앱 라이선스가 없으면 실행이 안 되므로(PowerShell 7에서 실제로 그랬다),
    /// **정식 설치본을 넣기 전에 먼저 지운다.**
    pub store_pkg: Option<&'static str>,
    /// 설명 i18n 키.
    pub desc: &'static str,
    /// **윈도우에서 쓸 수 없는 것**이면 그 이유 키. 되는 척하지 않는다.
    pub unavailable: Option<&'static str>,
}

/// GitHub 릴리스에서 **최신** MSI를 찾아 조용히 설치하는 뼈대.
///
/// 처음에는 `releases/latest/download/<고정이름>.msi`를 썼는데 **둘 다 404였다**(직접
/// 확인했다). GitHub의 그 경로는 파일 이름이 판마다 같을 때만 통하는데, PowerShell도
/// gh도 이름에 판 번호가 들어간다(`PowerShell-7.6.5-win-x64.msi`, `gh_2.98.0_windows_amd64.msi`).
/// 그래서 판을 우리가 박아 두면 다음 판이 나오는 날 조용히 깨진다 — API로 그때그때 찾는다.
const GH_MSI: &str = concat!(
    "$ProgressPreference='SilentlyContinue'; $ErrorActionPreference='Stop';\n",
    "Write-Output '@@STEP 1/4 resolve';\n",
    "$r=Invoke-RestMethod 'https://api.github.com/repos/{REPO}/releases/latest' -Headers @{'User-Agent'='nabiTerm'};\n",
    "$a=$r.assets | Where-Object { $_.name -like '{PAT}' } | Select-Object -First 1;\n",
    "if (-not $a) { throw 'no installer asset' }\n",
    "Write-Output '@@STEP 2/4 download';\n",
    "$f=Join-Path $env:TEMP $a.name;\n",
    "Invoke-WebRequest -Uri $a.browser_download_url -OutFile $f -UseBasicParsing;\n",
    "Write-Output '@@STEP 3/4 install';\n",
    "$p=Start-Process msiexec.exe -ArgumentList @('/i',$f,'/quiet','/norestart') -Wait -PassThru;\n",
    "[IO.File]::Delete($f);\n",
    "Write-Output '@@STEP 4/4 done';\n",
    "if ($p.ExitCode -ne 0 -and $p.ExitCode -ne 3010) { exit $p.ExitCode }\n",
);

/// `GHMSI:<owner/repo>:<자산 이름 패턴>` 명세를 실제 스크립트로 편다.
pub(crate) fn gh_msi_script(spec: &str) -> Option<String> {
    let (repo, pat) = spec.strip_prefix("GHMSI:")?.split_once(':')?;
    if repo.is_empty() || pat.is_empty() {
        return None;
    }
    Some(GH_MSI.replacen("{REPO}", repo, 1).replacen("{PAT}", pat, 1))
}

/// **winget 자체를 까는 스크립트.**
///
/// Windows Server에는 앱 설치 관리자가 없어 winget이 아예 없다(이 개발 PC가 그렇다).
/// winget이 있으면 나머지 도구가 전부 한 줄로 끝나므로 이것부터 클릭으로 깔 수 있게 하는
/// 편이 사용자에게 가장 이득이다(사용자 제안 2026-08-25).
///
/// ## 두 번 넘어지고 배운 것 (둘 다 직접 확인했다)
///
/// 1. 널리 도는 방법대로 VCLibs·UI.Xaml만 먼저 깔면 **0x80073CF3으로 튕긴다.** 요즘
///    winget은 `WindowsAppRuntime`까지 필요하고 그 목록은 판마다 바뀐다. 그래서 목록을
///    우리가 관리하지 않고 마이크로소프트가 릴리스에 같이 올리는
///    `DesktopAppInstaller_Dependencies.zip`을 그대로 쓴다.
/// 2. `Add-AppxPackage`만 하면 설치는 "성공"하고 PATH에도 올라가는데 **실행하면
///    "No applicable app licenses found"로 죽는다.** 즉 되는 척만 한다. 관리자라면
///    라이선스와 함께 `Add-AppxProvisionedPackage`로 넣어야 진짜로 돈다.
///
/// 그래서 마지막에 **실제로 실행해 본다.** 안 돌면 성공이라고 말하지 않는다.
const WINGET_PS: &str = concat!(
    "$ProgressPreference='SilentlyContinue'; $ErrorActionPreference='Stop';\n",
    "Write-Output '@@STEP 1/5 resolve';\n",
    "$r=Invoke-RestMethod 'https://api.github.com/repos/microsoft/winget-cli/releases/latest' -Headers @{'User-Agent'='nabiTerm'};\n",
    "$dep=$r.assets | Where-Object { $_.name -eq 'DesktopAppInstaller_Dependencies.zip' } | Select-Object -First 1;\n",
    "$pkg=$r.assets | Where-Object { $_.name -like '*.msixbundle' } | Select-Object -First 1;\n",
    "$lic=$r.assets | Where-Object { $_.name -like '*License1.xml' } | Select-Object -First 1;\n",
    "if (-not $dep -or -not $pkg) { throw 'winget release assets not found' }\n",
    "$d=Join-Path $env:TEMP ('nabi-winget-' + [guid]::NewGuid().ToString('N'));\n",
    "New-Item -ItemType Directory -Path $d | Out-Null;\n",
    "Write-Output '@@STEP 2/5 dependencies';\n",
    "$z=Join-Path $d 'deps.zip';\n",
    "Invoke-WebRequest -Uri $dep.browser_download_url -OutFile $z -UseBasicParsing;\n",
    "Expand-Archive -Path $z -DestinationPath (Join-Path $d 'deps') -Force;\n",
    "$arch=if ([Environment]::Is64BitOperatingSystem) { 'x64' } else { 'x86' };\n",
    "Write-Output '@@STEP 3/5 frameworks';\n",
    "Get-ChildItem -Path (Join-Path $d 'deps') -Recurse -Filter *.appx |\n",
    "  Where-Object { $_.FullName -like ('*' + $arch + '*') } |\n",
    "  ForEach-Object { Add-AppxPackage -Path $_.FullName -ErrorAction SilentlyContinue };\n",
    "Write-Output '@@STEP 4/5 download';\n",
    "$b=Join-Path $d $pkg.name;\n",
    "Invoke-WebRequest -Uri $pkg.browser_download_url -OutFile $b -UseBasicParsing;\n",
    "Write-Output '@@STEP 5/5 install';\n",
    "$admin=([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);\n",
    "if ($admin -and $lic) {\n",
    "  $l=Join-Path $d $lic.name;\n",
    "  Invoke-WebRequest -Uri $lic.browser_download_url -OutFile $l -UseBasicParsing;\n",
    "  Add-AppxProvisionedPackage -Online -PackagePath $b -LicensePath $l | Out-Null;\n",
    "} else { Add-AppxPackage -Path $b }\n",
    "Remove-Item -Path $d -Recurse -Force -ErrorAction SilentlyContinue;\n",
    "$exe=Join-Path $env:LOCALAPPDATA 'Microsoft/WindowsApps/winget.exe';\n",
    "$v = & $exe --version 2>&1;\n",
    "if ($LASTEXITCODE -ne 0) { throw ('winget installed but will not run: ' + $v) }\n",
);

/// 카탈로그 본체. 순서가 화면 순서다.
pub(crate) const TOOLS: &[Tool] = &[
    Tool {
        id: "winget",
        name: "Windows 패키지 관리자 (winget)",
        probe: "winget",
        group: Group::Pkg,
        // 자기 자신을 winget으로 깔 수는 없다.
        winget: None,
        fallback: Some(WINGET_PS),
        remove: None,
        store_pkg: None,
        desc: "env.desc.winget",
        unavailable: None,
    },
    Tool {
        id: "pwsh",
        name: "PowerShell 7",
        probe: "pwsh",
        group: Group::Shell,
        // winget 원본을 못 박는다 — msstore 원본이 잡히면 다시 스토어판이 깔린다.
        winget: Some("Microsoft.PowerShell"),
        store_pkg: Some("Microsoft.PowerShell"),
        fallback: Some("GHMSI:PowerShell/PowerShell:*win-x64.msi"),
        remove: None,
        desc: "env.desc.pwsh",
        unavailable: None,
    },
    Tool {
        id: "gh",
        name: "GitHub CLI (gh)",
        probe: "gh",
        group: Group::DevTool,
        winget: Some("GitHub.cli"),
        fallback: Some("GHMSI:cli/cli:*windows_amd64.msi"),
        remove: None,
        store_pkg: None,
        desc: "env.desc.gh",
        unavailable: None,
    },
    Tool {
        id: "ripgrep",
        name: "ripgrep (rg)",
        probe: "rg",
        group: Group::DevTool,
        winget: Some("BurntSushi.ripgrep.MSVC"),
        fallback: None,
        remove: None,
        store_pkg: None,
        desc: "env.desc.ripgrep",
        unavailable: None,
    },
    Tool {
        id: "fzf",
        name: "fzf",
        probe: "fzf",
        group: Group::DevTool,
        winget: Some("junegunn.fzf"),
        fallback: None,
        remove: None,
        store_pkg: None,
        desc: "env.desc.fzf",
        unavailable: None,
    },
    Tool {
        id: "jq",
        name: "jq",
        probe: "jq",
        group: Group::DevTool,
        winget: Some("jqlang.jq"),
        fallback: None,
        remove: None,
        store_pkg: None,
        desc: "env.desc.jq",
        unavailable: None,
    },
    Tool {
        id: "sshpass",
        name: "sshpass",
        probe: "sshpass",
        group: Group::DevTool,
        winget: None,
        fallback: None,
        remove: None,
        store_pkg: None,
        desc: "env.desc.sshpass",
        // 윈도우 네이티브 빌드가 없다. 되는 척하는 대신 대안을 안내한다.
        unavailable: Some("env.na.sshpass"),
    },
];

/// id로 찾는다(시험이 카탈로그를 짚을 때 쓴다).
#[cfg(test)]
pub(crate) fn find(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// id가 겹치면 버튼이 엉뚱한 도구를 설치한다 — 컴파일로는 안 잡힌다.
    #[test]
    fn ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for t in TOOLS {
            assert!(seen.insert(t.id), "id 중복: {}", t.id);
        }
    }

    /// 설치할 길이 하나도 없는데 "설치" 버튼이 보이면 안 된다.
    #[test]
    fn every_installable_tool_has_a_channel() {
        for t in TOOLS {
            if t.unavailable.is_some() {
                continue;
            }
            assert!(
                t.winget.is_some() || t.fallback.is_some(),
                "{}: 설치 통로가 없다",
                t.id
            );
        }
    }

    /// 진행바가 실제 진행을 따라가려면 스크립트가 단계를 말해야 한다.
    #[test]
    fn the_fallback_script_reports_its_steps() {
        let s = gh_msi_script("GHMSI:cli/cli:*windows_amd64.msi").unwrap();
        assert!(s.contains("@@STEP 1/4"));
        assert!(s.contains("@@STEP 4/4"));
        assert!(s.contains("repos/cli/cli/releases/latest"));
        assert!(s.contains("*windows_amd64.msi"));
        assert!(!s.contains("{REPO}") && !s.contains("{PAT}"), "치환이 남았다");
    }

    /// **판 번호를 박아 두면 다음 판이 나오는 날 404가 된다** — 실제로 둘 다 404였다.
    #[test]
    fn no_download_url_pins_a_version() {
        for t in TOOLS {
            let Some(f) = t.fallback else { continue };
            assert!(
                !f.contains("releases/latest/download/"),
                "{}: 고정 이름 내려받기 경로는 판이 바뀌면 깨진다",
                t.id
            );
        }
    }

    #[test]
    fn a_malformed_spec_is_rejected() {
        assert!(gh_msi_script("cli/cli:*.msi").is_none(), "접두사가 없다");
        assert!(gh_msi_script("GHMSI:cli/cli").is_none(), "패턴이 없다");
        assert!(gh_msi_script("GHMSI::*.msi").is_none(), "레포가 비었다");
    }

    #[test]
    fn find_locates_and_rejects() {
        assert_eq!(find("gh").map(|t| t.name), Some("GitHub CLI (gh)"));
        assert!(find("없는도구").is_none());
    }
}
