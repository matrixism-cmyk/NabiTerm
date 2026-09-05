# 🦋 nabiTerm

**한국어** → [README.ko.md](README.ko.md)

**A native Windows terminal multiplexer + MobaXterm-style SSH client** — fast, lightweight, and written from scratch in Rust. Single executable, no runtime dependencies.

**Homepage** → [nabisori.kr](https://nabisori.kr/nabiterm.php)

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/matrixism-cmyk/NabiTerm?filter=v*&label=release)](https://github.com/matrixism-cmyk/NabiTerm/releases)
[![CI](https://github.com/matrixism-cmyk/NabiTerm/actions/workflows/ci.yml/badge.svg)](https://github.com/matrixism-cmyk/NabiTerm/actions)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-informational)

> 📢 **2026-08-19 — nabiTerm is now open source!** The full source code is available here under
> Apache-2.0, and releases are published from this repository. Issues and PRs are welcome →
> [CONTRIBUTING.md](CONTRIBUTING.md)

![nabiTerm screenshot](docs/img/screenshot-main.png)

---

## Overview

nabiTerm is a terminal for working with local shells and remote servers in one window. Arrange sessions with tabs, splits, and tear-off windows; connect over SSH/SFTP/FTP; and use the built-in file browser, editor, and password vault — all in a single program. It ships as **one exe** with no external runtime, and keeps itself current with auto-update.

On top of a solid traditional core (robust VT engine, deep scrollback), it adds modern terminal features — command blocks, hyperlinks, inline images, styled underlines — and an **AI agent control plane**.

### The part you won't find elsewhere

An AI CLI running inside a pane can drive nabiTerm itself — list windows, read another
pane's screen, open a shell, send input, wait for it to finish, move files over SFTP:

```console
$ nabi cli list --json
[{"pane":1,"kind":"ssh","title":"web-01","state":"idle","cwd":"/srv/app"}]

$ nabi cli spawn --shell pwsh --dock split-down
pane 7
$ nabi cli send --pane 7 --data "cargo test"
$ nabi cli wait --pane 7 --idle 5
$ nabi cli capture --pane 7 --lines 12
test result: ok. 1601 passed; 0 failed
```

The same surface is exposed as an **MCP server**. Register it with Claude Code in one line:

```powershell
claude mcp add nabiterm -- "C:\Program Files\Nabisori\NabiTerm\nabiTerm.exe" mcp
```

16 tools — list panes, capture a screen, spawn a shell, send input, wait, close, and
**SFTP list / get / put**: the agent doesn't only run commands, it moves files over the
SSH session you already have open.

No network port is opened — it speaks over a Windows named pipe, and every verb is gated
by an **off / ask / on** permission policy that defaults to *ask*.

## Features

### Terminal & sessions
- **Tabs / splits / tear-off windows** — egui_dock docking, drag a tab out into its own OS window, tmux-style pane zoom
- **Local shells** — PowerShell, cmd, WSL, Git Bash (Windows ConPTY); restores your running command and directory after restart
- **SSH / SFTP / FTP** — pure-Rust async stack (russh), host-key TOFU verification, port forwarding (-L / -R / -D, ProxyJump, X11)
- **trzsz file transfer (`trz` / `tsz`)** — send and receive files from inside a live shell,
  where a separate SFTP channel isn't available (jump hosts, `sudo -i`, container `exec`, serial consoles).
  Every transfer is confirmed; remote-supplied names are refused, never sanitised
- **Broadcast input** — type into many panes at once
- **Search & scrollback** — smart-case find, scrollbar, prompt-to-prompt jumps
- **Quake mode, fullscreen, themes** — 8 color presets, cursor/selection color customization; UI in English / Korean / Japanese

### Files & editing
- **File browser** — Explorer-style detail/icon views, drag-out copy, This-PC drive overview, dual-pane local↔remote
- **Built-in editor (nabiPad)** — syntax highlighting (syntect), automatic encoding detection, HEX editor, virtualized viewer for huge files, LSP integration
- **Password vault** — SSH credentials encrypted with Argon2id + AES-256-GCM

### Modern terminal features
- **Warp-style command blocks** — OSC 133-based; exit-status color bar next to each prompt line
- **Hyperlinks** — heuristic URL/path detection plus explicit OSC 8 links, long-press menu (copy/open)
- **Inline images** — Sixel, iTerm (OSC 1337), and Kitty (APC) protocols *(over SSH; locally iTerm OSC only, a ConPTY limitation)*
- **Styled underlines** — undercurl, double, dotted, dashed, with underline colors (SGR 58 — e.g. nvim LSP diagnostics)

### AI agent integration
- **AI command bar** — when claude / codex / gemini / aider is running in a pane, its slash commands appear as clickable buttons above the terminal, with submenus (model, effort, …) and hover descriptions
- **AI terminal profiles** — save a shell + CLI + switches (e.g. `--dangerously-skip-permissions`) as a profile and launch it in one click
- **Control plane** — processes inside panes can drive nabiTerm via `nabi cli` (named pipe) or MCP: `list`, `spawn`, `send`, `capture`, `wait`, `notify`, and more, gated by an off / ask / on permission policy
- Help ▸ AI Control manages AI CLI installation and ships a copyable usage guide

## Installation

Grab it from [**Releases**](https://github.com/matrixism-cmyk/NabiTerm/releases).

- `nabiTerm-setup.exe` — installer (per-user, no admin rights required)

Deploying to many machines? The installer takes unattended switches:

```powershell
nabiTerm-setup.exe /VERYSILENT /NOLAUNCH            # install, don't start
nabiTerm-setup.exe /VERYSILENT /ALLUSERS /NOLAUNCH  # machine-wide (needs admin)
```

After installation, **auto-update** notifies you of new versions and applies them in one step (manual check in Help ▸ About).

> Installs at v0.1.446 or older check the legacy repository ([NabiTermPub](https://github.com/matrixism-cmyk/NabiTermPub/releases)). **Every release is published to both**, so simply updating brings you onto this repository.

### Building from source

See `BUILD.md` — nabiTerm builds with the GNU toolchain (MinGW-w64), no MSVC required. In short: `rustup default stable-gnu`, put MinGW-w64 on `PATH`, then `cargo build --release -p nabi-app`.

### GPU-less VMs & headless servers

nabiTerm renders with the GPU (wgpu: DX12 → Vulkan → GL). On machines with **no GPU at all**, it self-checks at startup:

- **Online**: a prompt offers to download the software-rendering runtime (~22 MB, SHA-256 verified) next to `nabiTerm.exe`; after that it starts directly.
- **Offline / air-gapped**: download the pinned [Mesa runtime asset](https://github.com/matrixism-cmyk/NabiTerm/releases/download/mesa-runtime/nabiTerm-mesa-software-gl.zip) (~22 MB) and unzip the two DLLs next to `nabiTerm.exe`.
- `NABI_RENDERER=software` forces software rendering; `NABI_RENDERER=hardware` skips the check.

## Tech stack

- **Language:** Rust (workspace of many `nabi-*` crates)
- **Terminal core:** alacritty_terminal · **GUI:** egui / eframe (wgpu: DX12→Vulkan→GL, software-GL fallback)
- **SSH/SFTP:** russh / russh-sftp · **Local PTY:** portable-pty (ConPTY)
- **Platform:** Windows 10 / 11 (x64)

## License & contributing

nabiTerm is open source under the **Apache License 2.0** (see [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE)).

- **Source & releases**: <https://github.com/matrixism-cmyk/NabiTerm>
- **Contributing**: issues and PRs welcome — see [`CONTRIBUTING.md`](CONTRIBUTING.md) (DCO sign-off)
- **Security reports**: see [`SECURITY.md`](SECURITY.md)
- The "nabiTerm / 나비텀" name and logo are not covered by the license (Apache-2.0 §6 — no trademark grant).

Third-party components and their licenses are listed in-app under **Help ▸ Open Source** and below. `vendor/russh-sftp-2.3.0` is a modified vendored copy (SFTP filename-encoding support); changes are documented in `NOTICE` and in source comments.

## Third-party notices

nabiTerm is built on open source — thanks to all the authors. Most of the ~660 transitive dependencies are under permissive MIT/Apache-2.0 licenses.

| Component | Purpose | License |
|---|---|---|
| alacritty_terminal | terminal / VT core | Apache-2.0 |
| egui · eframe · epaint · egui_extras | GUI framework | MIT OR Apache-2.0 |
| wgpu | GPU rendering backend | MIT OR Apache-2.0 |
| Mesa 3D (llvmpipe, separate asset) | software OpenGL fallback | MIT |
| egui_dock | docking tabs | MIT |
| epaint_default_fonts | bundled UI fonts | OFL-1.1, UFL-1.0 |
| image | PNG/JPEG/GIF decode | MIT OR Apache-2.0 |
| portable-pty | local PTY (ConPTY) | MIT |
| russh · russh-sftp | SSH / SFTP | Apache-2.0 |
| suppaftp | FTP | MIT OR Apache-2.0 |
| tokio | async runtime | MIT |
| encoding_rs · chardetng | text encoding / detection | Apache-2.0/MIT, BSD-3 |
| syntect · fancy-regex | syntax highlighting | MIT |
| ttf-parser | font enumeration | MIT OR Apache-2.0 |
| memmap2 | huge-file viewer | MIT OR Apache-2.0 |
| arboard · rfd | clipboard · file dialogs | MIT (OR Apache-2.0) |
| argon2 · aes-gcm · zeroize | vault encryption | MIT OR Apache-2.0 |
| serde · serde_json · toml | config / serialization | MIT OR Apache-2.0 |
| chrono · directories | time · paths | MIT OR Apache-2.0 |
| option-ext (via directories) | transitive dependency | MPL-2.0 |

---

<sub>🤖 nabiTerm is developed with help from [Claude Code](https://claude.com/claude-code) — including this document.</sub>
