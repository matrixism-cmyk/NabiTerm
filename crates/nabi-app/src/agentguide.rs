//! AI 에이전트(클로드 코드 등)에게 건네줄 제어 평면 사용설명(Markdown).
//! 도움말 ▸ AI 제어에서 "복사" 또는 "MD로 저장"으로 제공한다(영문 — LLM 범용).

/// 실행 중인 nabiTerm의 실제 exe 경로를 주입한 사용설명을 만든다(복사/저장 공용).
/// `__NABI_EXE__` 자리표시자를 현재 경로로 치환해 AI가 PATH 없이도 정확히 호출하게 한다.
pub(crate) fn agent_guide_md(exe_path: &str) -> String {
    AGENT_GUIDE_MD.replace("__NABI_EXE__", exe_path)
}

/// pane 안의 AI가 nabiTerm을 제어하는 방법을 정리한 Markdown 가이드 템플릿.
const AGENT_GUIDE_MD: &str = r#"# Controlling nabiTerm from inside a pane

You are an AI agent (e.g. Claude Code) running inside a nabiTerm terminal pane.
nabiTerm exposes a local control plane so you can open, inspect and drive other
panes — spawn build/test shells, watch their output, send input, notify the user.

## How to call it

The control CLI is the same executable that runs nabiTerm's GUI. On THIS machine it is:

    __NABI_EXE__

In the examples below, `nabi` means exactly that file (nabiTerm's CLI). It is normally
on PATH inside a pane; if `nabi` is not found, call it by full path instead, e.g.
`& "__NABI_EXE__" cli list`.

It connects to nabiTerm over a local named pipe, prints the result, and exits:

    nabi cli <verb> [options]

Discovery is automatic — nabiTerm sets these env vars in every pane, so `nabi cli`
already knows how to reach the app and which pane you are:

- `NABI_CONTROL_PIPE`  — named pipe to connect to
- `NABI_CONTROL_TOKEN` — auth token (sent for you)
- `NABI_PANE_ID`       — your own pane id

Add `--json` to any verb for machine-readable output.

## MCP server (alternative — native tool integration)

Instead of shelling out to `nabi cli`, you can register nabiTerm as an MCP server so your
MCP client (Claude Code, Cursor, …) gets the same pane tools natively. From inside a pane:

    claude mcp add nabiterm -- "__NABI_EXE__" mcp

This runs a stdio JSON-RPC MCP server exposing the same actions as the verbs below
(nabi_list_panes / nabi_capture / nabi_spawn / nabi_send / nabi_wait / nabi_kill /
nabi_focus / nabi_notify / nabi_open_browser / nabi_open_sftp / …). It uses the same
control pipe, so the permission policy (off/ask/on) applies identically — MCP is not a bypass.

## Verbs

### Inspect (always allowed)

- `nabi cli list [--json]`
  List panes: id, title, size, kind (local/ssh), cwd, state, last exit code.
- `nabi cli capture --pane <id> [--lines <n>] [--start <l> --end <l>] [--escapes]`
  Read a pane's screen/scrollback. `--lines 100` = last 100 lines; `--escapes` keeps ANSI colors.
- `nabi cli wait --pane <id> --until exit|command-done|idle|output [--match <text> | --regex <pat>] [--timeout <ms>]`
- `nabi cli agent report --state working|blocked|idle|done` / `agent release`
  Publish your own state (authoritative — screen detection steps aside). Call from hooks/statusLine.
- `nabi cli agent wait --pane <id> --until idle|working|blocked|done [--timeout <ms>]`
  Block until the agent in that pane reaches a state (screen-detected or hook-published).
- `nabi cli agent prompt --pane <id> --data <text> [--wait [--until <state>]] [--timeout <ms>]`
  Type a prompt into another agent pane (Enter included) and optionally wait for its state.
- `nabi cli agent explain --pane <id>` — why the state detector classified a pane as it did.
  Block until the pane finishes its command / goes idle / outputs. Default timeout 60000 ms.
- `nabi cli tail --pane <id>`
  Stream a pane's output continuously.

### Act (may require one-time approval in "ask" mode)

- `nabi cli spawn [--shell powershell|pwsh|cmd|wsl|gitbash] [--cwd <path>] [--dock tab|split-right|split-down|new-window] [--ssh <session>]`
  Open a new pane. Prints the new pane id — use it in later commands.
  `--shell` default is `powershell` (Windows PowerShell 5.1, always present). `pwsh` is
  PowerShell 7 (only if installed). If the chosen shell isn't installed, spawn returns a
  clear error immediately (naming the missing executable, e.g. pwsh.exe) instead of hanging.
- `nabi cli focus --pane <id>` — bring a pane's tab to front.
- `nabi cli set-title --pane <id> --title <text>` — rename a pane's tab.
- `nabi cli resize --pane <id> --cols <c> --rows <r>`
- `nabi cli notify --title <text> [--body <text>]` — desktop/toast notification.
- `nabi cli open-browser [--path <dir>]` — open the file browser.

### Inject (separate approval in "ask" mode)

- `nabi cli send --pane <id> --data <text> [--raw]`
  Type into a pane. Append a carriage return to press Enter (PowerShell: "cmd`r").
  Default wraps as a bracketed paste; `--raw` sends bytes verbatim (control keys).
- `nabi cli kill --pane <id>` — close a pane.
- `nabi cli open-sftp --session <name>` — open an SFTP browser for a saved session.

## Targeting a pane without an id

Instead of `--pane <id>` you can match by property:

    nabi cli capture --match "title:build,state:idle"
    # keys: title (substring), cwd (prefix), kind, state, id
    # prefix a value with ! to negate, e.g. --match "kind:ssh,state:!idle"

## Permissions

Control mode lives in nabiTerm ▸ Settings ▸ Behavior ▸ Agent control: off / ask / on.

- off  — all control is refused.
- ask  — (default) Inspect verbs always work. The first Act and the first Inject
         in this nabiTerm instance pop an approval dialog; the user clicks Allow once
         (Act and Inject are approved separately).
- on   — everything is allowed without prompting.

If a command returns an "approval pending" error, ask the user to approve it in
nabiTerm, then retry.

## Publishing your status to nabiTerm (status bar + tab)

You can surface live info — model, token usage, current activity — in nabiTerm's status
bar and tab badge. Emit an OSC 7771 escape with the `status-set` verb and a JSON {key,value}:

    printf '\033]7771;status-set;{"key":"model","value":"claude-opus-4.8"}\033\\'
    printf '\033]7771;status-set;{"key":"tokens","value":"42k/200k"}\033\\'
    printf '\033]7771;status-set;{"key":"state","value":"thinking"}\033\\'

Clear one key, or all keys for this pane, with `status-clear`:

    printf '\033]7771;status-clear;{"key":"state"}\033\\'   # one key
    printf '\033]7771;status-clear;{}\033\\'                 # all keys (this pane)

From PowerShell use the same escape via `` `e `` (PS7) or `[char]27`:

    $e=[char]27; Write-Host "$e]7771;status-set;{`"key`":`"tokens`",`"value`":`"42k/200k`"}$e\" -NoNewline

This needs Settings ▸ Behavior ▸ Agent control to allow in-band OSC, and only works for
local panes (remote/SSH panes are ignored).

Alternatively (more reliable — no terminal rendering needed), publish via the control CLI:

    nabi cli status set model "claude-opus-4.8"
    nabi cli status set tokens "42k/200k"     # used/limit → context gauge + 80/95% warnings
    nabi cli status set cost "$1.40"           # → AI dashboard total + per-agent cost
    nabi cli status set state "waiting"        # "waiting"/"input"/"blocked" → input-needed toast
    nabi cli status clear state                 # clear one key (omit key = clear all for this pane)
    nabi cli status set state working --ttl 60000   # auto-clears after 60s (TTL)
    nabi cli status set label.working "refactoring auth"   # per-state label shown in AI dashboard

### Claude Code statusLine integration (recommended)

Wire it into Claude Code's `statusLine` so model/tokens/cost refresh every turn and show up in
nabiTerm's status bar, tab badge, and AI usage dashboard. Save this script and point
settings.json `"statusLine": { "type": "command", "command": "bash ~/.claude/nabi-status.sh" }`:

    #!/usr/bin/env bash
    in=$(cat)                                   # Claude Code feeds session JSON on stdin
    model=$(printf '%s' "$in" | jq -r '.model.display_name // .model.id // "?"')
    nabi cli status set model "$model" 2>/dev/null
    # Optional: ccusage gives tokens/cost — forward them too:
    #   nabi cli status set cost "$(printf '%s' "$in" | ccusage statusline --field costUSD 2>/dev/null)"
    printf '%s' "$model"                         # also Claude Code's own status line text

PowerShell equivalent (`command`: `pwsh -File ~/.claude/nabi-status.ps1`):

    $in = $input | Out-String
    $model = ($in | ConvertFrom-Json).model.display_name
    nabi cli status set model "$model" 2>$null
    $model

When the pane exits, nabiTerm clears its status automatically.

## Typical workflow (PowerShell)

    $pane = (nabi cli spawn --shell powershell --cwd C:\proj --dock split-right --json | ConvertFrom-Json).pane
    nabi cli send --pane $pane --data "cargo test`r"
    nabi cli wait --pane $pane --until command-done --timeout 600000
    nabi cli capture --pane $pane --lines 200
    nabi cli notify --title "Tests finished"

Notes: `nabi` is the same exe that runs the GUI; the `cli` subcommand does one
request and exits. Pane ids are integers from `nabi cli list`.
"#;
