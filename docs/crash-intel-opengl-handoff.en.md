# nabiTerm Launch-Crash Diagnosis Handoff (Intel HD 530 / OpenGL)

> Written: 2026-06-27 · Target machine: user PC (Windows 11 Pro, Intel HD Graphics 530)
> Recipient: the Claude Code (dev agent) that authored nabiTerm's source
> Status: **Root cause confirmed.** Not a source bug → render-backend policy decision needed.

---

## TL;DR

- Symptom: nabiTerm **intermittently crashes right at launch / during use** (felt as "won't run"). Not always — some sessions run fine (during diagnosis, PID 11584 survived 16+ hours).
- Cause: eframe's **default glow (OpenGL) renderer** → this PC's **Intel HD 530 OpenGL driver (`ig9icd64.dll`)** faults internally with `ACCESS_VIOLATION (0xC0000005)`.
- HD 530 is **end-of-life (Skylake / 6th gen)**; the installed `30.0.101.1339` (2022-01-21) is **effectively the last driver** → **cannot be fixed by a driver update.**
- Not a nabiTerm source defect. The fix is to **detach the render path from the OpenGL ICD.**

---

## Evidence Collected

### 1) Two crash dumps (WER LocalDumps)
Path: `%LOCALAPPDATA%\CrashDumps\`

| Dump | Time | Exception code | Fault location |
|---|---|---|---|
| `nabiTerm.exe.17004.dmp` | 2026-06-27 10:08 | `0xC0000005` ACCESS_VIOLATION | **inside `ig9icd64.dll`** (base+0xAF00E0) |
| `nabiTerm.exe.6824.dmp` | 2026-06-20 12:21 | `0xC0000005` ACCESS_VIOLATION | addr `0x0` (null deref, GPU/render thread) |

> `ig9icd64.dll` = Intel Graphics **OpenGL ICD** (Installable Client Driver).
> The fault address lies inside this module's code region → **the driver itself dies while servicing an OpenGL call.**

Minidump parsing was done directly in PowerShell (cdb not installed): extract ExceptionCode and
ExceptionAddress from the exception stream (type 6), then reverse-map that address to the containing
module via the ModuleList (type 4).

### 2) GPU / driver
```
Name          : Intel(R) HD Graphics 530
DriverVersion : 30.0.101.1339
DriverDate    : 2022-01-21
Status        : OK   (not a HW failure)
```
OpenGL runtime log (captured from a healthy session):
```
opengl version : 3.3.0 - Build 30.0.101.1339
opengl renderer: Intel(R) HD Graphics 530
opengl vendor  : Intel
Shader version : Gl140 ("3.30")
eframe         : Using the glow renderer
```

### 3) Healthy-launch log (reference — init itself passes)
glow Display creation → GL context (WGL) → keyring (Windows Credentials) → control server
(`\\.\pipe\nabi-control-<pid>`) → orchestrator → alacritty_terminal, all entered normally.
So **the app logic is fine**; the crash point is mid-way through OpenGL driver calls.

---

## Root Cause

eframe runs on the **glow (OpenGL) backend** (`nabi-app` Cargo feature `"glow"`, default
`Renderer::Glow` in `NativeOptions`), and the actual GL commands flow into the **aged OpenGL ICD of
the Intel HD 530**, which crashes inside it on a memory-access violation. The ICD is the final build
for an EOL chipset, so there is no patch.

- This is an SEH exception raised in a foreign (driver) module, so it **cannot be caught by Rust's
  panic hook / `catch_unwind`** → `nabi_log::install_crash_handler` (main.rs:250) cannot recover
  gracefully; the process dies instantly.
- Intermittency: occurs only when GL state / timing / resize, etc. drive the ICD down a fatal path.

### Relevant source locations
- `crates/nabi-app/src/main.rs:260` — `NativeOptions { ... }` (renderer unspecified → glow default).
- `crates/nabi-app/Cargo.toml` — `eframe = { ... features = ["accesskit","default_fonts","glow","persistence"] }`
- Workspace `Cargo.toml` comment: `# M1 simplified to the glow(OpenGL) backend; wgpu custom renderer is a follow-up.`
- No glow-specific paint callbacks (`egui_glow` / `PaintCallback` / `glow::`) are used → **no risk of
  breaking custom render code** when swapping the backend (only immediate-mode egui is used).

---

## Remediation Options (pick one or combine)

### A. Drop in Mesa3D `opengl32.dll` — no rebuild · immediate · reversible  ★quick mitigation
Ship Mesa3D's `opengl32.dll` in the install folder (`%LOCALAPPDATA%\Programs\nabiTerm\`) so glow loads
**Mesa** instead of the Intel ICD → fully bypasses the faulting `ig9icd64.dll` path.
- `GALLIUM_DRIVER=llvmpipe` : pure software rendering. **Most stable** (eliminates the driver crash at
  the source). A terminal emulator performs fine on software rendering.
- `GALLIUM_DRIVER=d3d12` : GL→DirectX12 translation. Keeps GPU acceleration while avoiding the OpenGL ICD.
- Pros: zero source change, rollback is just removing the DLL, verifiable immediately on the current build.
- Work: include the DLL + default env var in the installer (`installer/nabiTerm.iss`) and portable-zip packaging.
- Note: add the Mesa build's (mesa-dist-win, etc.) license notice (MIT).

### B. Switch to eframe **wgpu (DX12)** renderer — the proper fix · requires rebuild
- `Cargo.toml`: eframe feature `"glow"` → `"wgpu"`.
- `main.rs`: `NativeOptions { renderer: eframe::Renderer::Wgpu, ..}`.
- Removes the OpenGL path itself (wgpu's default DX12 backend on Windows).
- Trade-offs: larger binary/deps, **this aged driver's DX12 is also not 100% guaranteed safe**, and the
  plan reserves GPU-renderer changes for "after explicit agreement" (item B4/P4).
- Must pass the gate: gnu/MinGW build + clippy 0 + xtask lines 0 + tests → dist → release (+0.0.1).

### C. (recommended in parallel) Expose a runtime fallback option
- Let the user force the renderer (glow / wgpu / mesa-software) via env var / setting.
- If keeping glow: `hardware_acceleration` fallback, keep multisampling at 0, etc. — but note that since
  the crash is **inside the driver, options alone cannot fully eliminate it** (state the mitigation limit).
- Consider self-recovery (safe mode) in `install_crash_handler`: "if the previous run crashed in GL,
  the next launch auto-selects software rendering."

### D. Not viable — driver update
HD 530 is EOL; `30.0.101.1339` (2022) is the last. No newer driver exists. **Not an option.**

---

## Recommended Path

1. **Now:** A (Mesa `opengl32.dll`, start with `llvmpipe`) to stabilize the user's PC — no rebuild, reversible.
2. **Real fix:** land B (wgpu/DX12) or C's safe-mode fallback in the next release (+0.0.1), but measure wgpu
   stability on this machine before choosing the default. If uncertain, keep A as the default fallback.
3. Either way, **expose a user-selectable renderer toggle** to cover aged-GPU users broadly.

---

## How to Verify

- Repro is intermittent — do not declare "fixed" from a single clean launch. Baseline on:
  - Monitor WER dumps: whether new `%LOCALAPPDATA%\CrashDumps\nabiTerm.exe.*.dmp` appear.
  - Repeat many cold starts + resize / many-tab / scrollback-stress scenarios.
  - No dumps for several days after the change = signal of resolution.
- Reproduce the dump analysis (no debugger): minidump header (`MDMP`) → from the StreamDirectory parse
  ExceptionStream(6)/ModuleList(4) → reverse-map the exception address into a module range.
  (With cdb/WinDbg installed, `!analyze -v` confirms the same conclusion.)

---

## Appendix — Field State Notes

- Rust toolchain: `stable-x86_64-pc-windows-gnu` (cargo/rustc 1.96.0) in `~/.cargo/bin`. Rebuild possible.
- Build artifacts: `Desktop\nabi\dist\` (setup/portable/standalone); installed exe 24.7 MB (built 2026-06-27 01:39).
- A running instance (PID 11584) survived through diagnosis → proves the app does not crash "every" time.
