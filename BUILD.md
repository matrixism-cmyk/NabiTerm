# 빌드 가이드 (Windows)

nabi는 네이티브 Windows 앱이다. 이 저장소는 **`x86_64-pc-windows-gnu`** 툴체인 + **MinGW-w64**로
빌드·검증되었다(관리자 권한 불필요, 사용자 영역 설치). MSVC 툴체인(Visual Studio Build Tools +
Windows SDK)으로도 빌드 가능하지만 아래는 검증된 무권한 경로다.

## 1. Rust 툴체인 (rustup, gnu 호스트)

```powershell
# rustup-init 다운로드 후 사용자 영역 설치 (gnu 호스트, default 프로필)
Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe" -UseBasicParsing
& "$env:TEMP\rustup-init.exe" -y --default-host x86_64-pc-windows-gnu --default-toolchain stable --profile default --no-modify-path
```

검증 시점 버전: rustc/cargo **1.96.0**.

## 2. MinGW-w64 (dlltool/gcc/ld) — 필수

gnu 툴체인은 순수 Rust는 자체 링커(rust-lld)로 링크하지만, `windows-sys` 등 **Windows API를
바인딩하는 크레이트는 `dlltool`이 필요**하다(가져오기 라이브러리 생성). portable-pty·egui·wgpu·
russh가 모두 해당하므로 MinGW-w64 binutils가 반드시 있어야 한다.

WinLibs 배포본(UCRT, gcc 16.x)을 받아 사용자 폴더에 푼다:

```powershell
$url = "https://github.com/brechtsanders/winlibs_mingw/releases/download/16.1.0posix-14.0.0-ucrt-r2/winlibs-x86_64-posix-seh-gcc-16.1.0-mingw-w64ucrt-14.0.0-r2.zip"
Invoke-WebRequest -Uri $url -OutFile "$env:TEMP\winlibs.zip" -UseBasicParsing
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::ExtractToDirectory("$env:TEMP\winlibs.zip", "$env:USERPROFILE")
# → C:\Users\<you>\mingw64\bin 에 gcc.exe / dlltool.exe / ld.exe
```

(최신 자산 URL은 https://github.com/brechtsanders/winlibs_mingw/releases 에서 확인.)

## 3. PATH 설정 후 빌드

cargo 실행 시 PATH에 MinGW bin과 cargo bin이 있어야 한다:

```powershell
$env:Path = "$env:USERPROFILE\mingw64\bin;$env:USERPROFILE\.cargo\bin;" + $env:Path

cargo build --workspace        # 전체 빌드
cargo run -p nabi-app          # 앱 실행
cargo test --workspace         # 테스트
cargo run -p xtask -- lines    # 라인 수 게이트(소프트 250 / 하드 400)
```

영구 적용하려면 두 경로를 사용자 PATH 환경변수에 추가한다.

## 참고

- 계획 문서의 버전 핀(egui 0.34 등)은 목표값이다. 현재 워크스페이스는 **검증된 호환 버전**으로
  핀되어 있다(`Cargo.toml`의 `[workspace.dependencies]` 주석 참조). 상향 시 한 곳만 수정한다.
- MSVC 툴체인을 쓰려면 `rustup default stable-x86_64-pc-windows-msvc` 후 VS Build Tools(C++ 워크로드
  + Windows SDK)를 설치한다. 이 경우 MinGW는 불필요하다.
