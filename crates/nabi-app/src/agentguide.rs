//! AI 에이전트(클로드 코드 등)에게 건네줄 제어 평면 사용설명(Markdown).
//! 도움말 ▸ AI 제어에서 "복사" 또는 "MD로 저장"으로 제공한다(영문 — LLM 범용).

/// 실행 중인 nabiTerm의 실제 exe 경로를 주입한 사용설명을 만든다(복사/저장 공용).
/// `__NABI_EXE__` 자리표시자를 현재 경로로 치환해 AI가 PATH 없이도 정확히 호출하게 한다.
pub(crate) fn agent_guide_md(exe_path: &str) -> String {
    AGENT_GUIDE_MD.replace("__NABI_EXE__", exe_path)
}

/// pane 안의 AI가 nabiTerm을 제어하는 방법을 정리한 Markdown 가이드 템플릿.
pub(crate) const AGENT_GUIDE_MD: &str = r#"# Controlling nabiTerm from inside a pane

You are an AI agent (e.g. Claude Code) running inside a nabiTerm terminal pane.
nabiTerm exposes a local control plane so you can open, inspect and drive other
panes — spawn build/test shells, watch their output, send input, notify the user.

## What nabiTerm is (read this first)

nabiTerm is a Windows terminal that is also an SSH client, an SFTP file manager, a code
editor (nabiPad) and a web browser — all in one window, all reachable from this CLI. Knowing
that changes what you should do: when a task needs a file read, a page viewed, or a remote
directory listed, you do **not** have to do it inside your own shell. Hand it to nabiTerm.

| If you want to...              | Don't                      | Do instead                      |
|--------------------------------|----------------------------|---------------------------------|
| show the user a long file      | `cat` it into your pane     | `nabi cli open-file --path ...` |
| run a build and watch it       | block your own shell        | `spawn` + `wait` + `capture`    |
| know what is actually on screen| guess from text             | `nabi cli screenshot`           |
| look at a web page / a web UI  | curl and parse HTML         | `nabi cli web --url ...`        |
| browse a remote server         | shell out to `sftp`         | `open-sftp` + `sftp-list`       |
| tell the user something        | print and hope they look    | `nabi cli notify`               |
| show how far along you are     | say "almost done"           | `nabi cli progress --pct 60`    |

Your pane is one of many. Other panes may hold other agents, builds, SSH sessions or the
user's own shell. `nabi cli list` is how you find out what exists before you touch anything.

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

Every verb below is marked with the permission tier the app actually enforces:

- `(read)`   — looks at something and changes nothing. Always allowed, in every mode.
- `(act)`    — changes something. In `ask` mode the first one asks the person once.
- `(inject)` — types into a pane, moves a file, or cannot be undone. Asked separately.
- `(local)`  — never reaches nabiTerm at all; it runs in your own process and prints.

**Plan around these marks.** They are not a description someone wrote — a test compares every
line here against the permission code, so a verb marked `(read)` will not stop and ask, and one
marked `(inject)` will, unless the mode is `on`. The headings below only group verbs by what
they are for; the mark on the line is what decides.

### Looking around

- (read) `nabi cli list [--json]`
  List panes: id, title, size, kind (local/ssh), cwd, state, last exit code.
- (read) `nabi cli capture --pane <id> [--lines <n>] [--start <l> --end <l>] [--escapes]`
  Read a pane's screen/scrollback. `--lines 100` = last 100 lines; `--escapes` keeps ANSI colors.
- (act) `nabi cli screenshot [--pane <id>] [--out <path.png>]`
  Save a PNG of the window, or of one pane's area. `capture` gives you **text**; this gives you
  **pixels** — use it when text cannot tell you what you need: did the image render, is the
  colour right, is the layout broken, did the built-in browser actually load the page.
  Without `--out` it writes to the temp folder. **It answers only once the file is written,
  and the answer is the path** — so you can read that file immediately. If it fails you get
  the reason, not a false success.
- (read) `nabi cli wait --pane <id> --until exit|command-done|idle|output [--match <text> | --regex <pat>] [--timeout <ms>]`
- (local) `nabi cli integration install claude` — auto-install a SessionStart hook that reports the
  session id, so workspace restore resumes the exact session (`claude --resume <id>`).
- (act) `nabi cli agent report --state working|blocked|idle|done` / `agent release`
- (act) `nabi cli agent session <id>` — report your session id (hooks call this).
  Publish your own state (authoritative — screen detection steps aside). Call from hooks/statusLine.
- (read) `nabi cli agent wait --pane <id> --until idle|working|blocked|done [--timeout <ms>]`
  Block until the agent in that pane reaches a state (screen-detected or hook-published).
- (inject) `nabi cli agent prompt --pane <id> --data <text> [--wait [--until <state>]] [--timeout <ms>]`
  Type a prompt into another agent pane (Enter included) and optionally wait for its state.
- (read) `nabi cli agent explain --pane <id>` — why the state detector classified a pane as it did.
- (read) `nabi cli events [--pane <id>] [--kind spawned,exit,output,command-done,agent-status,cwd]`
  Stream events as they happen (no replay). `nabi cli api schema` prints the full protocol doc.
- (act) `nabi cli open-file --path <file>`
  Open a file in nabiPad, the built-in editor. Use it instead of dumping a long file into the
  terminal — the person can then read, search and edit it properly.
- (act) `nabi cli open-here --path <dir>`
  Open a new terminal in that folder and bring the window to the front.
- (act) `nabi cli scroll --pane <id> [--lines <+past/-recent>] [--top] [--bottom]`
  Move a pane's scrollback the way a person turns the wheel, and report where it landed
  (`offset / history`). Pair it with `screenshot` when you need to see what a person sees —
  `capture` gives you the stored text, this gives you the view.
- (local) `nabi cli api schema`
  Print the full control protocol as JSON: every verb, its parameters, and one line on what
  it does. Read this when a verb below is not enough.
- (read) `nabi cli pane-modes --pane <id>`
  Read the terminal modes of a pane (alternate screen, mouse reporting, bracketed paste...).
  Useful when a pane behaves oddly: a full-screen program leaves different modes set.
- (act) `nabi cli status set <key> <value>` / `nabi cli status clear [key]`
  Publish your own state — model, tokens, cost, what you are doing — into nabiTerm's status
  bar and tab badge. See "Publishing your status" below for the keys that mean something.
- (act) `nabi cli progress --pane <id> [--pct <0-100>]`
  Show a progress badge in the status bar for that pane. Leave `--pct` out to clear it.
  nabiTerm also reads progress off the screen for cargo/cmake/pytest/docker, but telling it
  directly is exact — a long job you drive yourself should report its own progress.
- (act) `nabi cli web [--url <url>] [--window]`
  Open the built-in web browser **as a tab**, like any other tab: it has a title, it can be
  split, reordered and closed, and it comes back with the workspace. `--window` opens it as
  a separate OS window instead. Handy right after `forward` — you can pull a remote web UI
  through the SSH tunnel and look at it without leaving nabiTerm.
  (`open-browser` is the **file** browser; this one is the web.)
  Needs the Edge WebView2 runtime; if it is missing you get told how to install it.
  Combine it with `screenshot` to actually see what the page rendered.
- (act) `nabi cli web-list`
  List the open web tabs as JSON: `[{"pane":N,"url":"...","title":"..."}]`. Web tabs are
  UI-only, so they do NOT appear in `nabi cli list` — this is how you find their numbers.
- (inject) `nabi cli web-eval [--pane <id>] --js <code>`
  Run JavaScript inside a web tab and get the result back as JSON (Inject approval).
  This is how you READ and DRIVE the web without leaving nabiTerm:

      nabi cli web --url https://example.com
      nabi cli web-eval --js "document.title"
      nabi cli web-eval --js "document.body.innerText.slice(0, 4000)"
      nabi cli web-eval --js "document.querySelector('a.next').click()"

  The result is the value of the last expression, JSON-serialized (strings arrive quoted;
  `undefined` arrives as `null`). Keep extractions small and targeted — pull the element you
  need, not the whole page. With one web tab open `--pane` is optional; with several,
  `web-list` tells you which number to target. A tab that has never been shown has no page
  yet — focus it once first.
- (act) `nabi cli history [--pane <id>]`
  Show that pane's **full history** on screen — the same overlay the user gets by scrolling
  up. Programs that redraw in place (Claude Code, vim, top) leave only fragments in the
  terminal scrollback, but the session recording has everything. Use this when the person
  asks "what happened earlier" and the answer is longer than a capture.
- (act) `nabi cli web-text [--pane <id>]`
  The readable text of the page (`document.body.innerText`), as a JSON string. This is the
  one you want most of the time — reading a page beats curling it and parsing HTML, because
  what you get is what a person sees after scripts have run.
- (act) `nabi cli web-goto --url <url> [--pane <id>]` / `web-back` / `web-forward` / `web-reload` /
  `web-stop`
  Drive navigation. `web-back` and `web-forward` fail with a clear message when there is
  nowhere to go, instead of silently doing nothing.
- (act) `nabi cli web-shot [--out <file.png>] [--pane <id>]`
  A PNG of what the page currently shows. Without `--out` it writes to the temp folder and
  the reply names the file. Use this to check rendering — `web-text` cannot tell you whether
  an image loaded or a layout broke.
- (act) `nabi cli web-pdf [--out <file.pdf>] [--pane <id>]`
  The **whole** page as a PDF, not just the visible part — use it to keep a long page.
- (act) `nabi cli web-zoom --set <factor> [--pane <id>]`
  Zoom the page (1.0 = 100%, clamped to 0.25–5.0). Useful before `web-shot` when you want
  more of a long page in one image.
- (act) `nabi cli schedule create "<cron|every 15m|at 09:30>" --send <text>|--command <cmd>|--notify <text> [--pane-title <t>]`
  Register a recurring job (runs inside nabiTerm; survives restarts).
- (act) `nabi cli layout export` / `nabi cli layout apply --file <json>` — snapshot the tab layout
  (panes with cwd/command) and re-create a working set declaratively.
- (local) `nabi cli security audit [--json]` — report risky permission combinations (report-only).
  Block until the pane finishes its command / goes idle / outputs. Default timeout 60000 ms.
- (read) `nabi cli tail --pane <id>`
  Stream a pane's output continuously.

### Driving panes, tabs and the browser

- (act) `nabi cli spawn [--shell powershell|pwsh|cmd|wsl|gitbash] [--cwd <path>] [--dock tab|split-right|split-down|new-window] [--ssh <session>]`
  Open a new pane. Prints the new pane id — use it in later commands.
  `--shell` default is `powershell` (Windows PowerShell 5.1, always present). `pwsh` is
  PowerShell 7 (only if installed). If the chosen shell isn't installed, spawn returns a
  clear error immediately (naming the missing executable, e.g. pwsh.exe) instead of hanging.
- (act) `nabi cli focus --pane <id>` — bring a pane's tab to front.
- (act) `nabi cli set-title --pane <id> --title <text>` — rename a pane's tab.
- (act) `nabi cli resize --pane <id> --cols <c> --rows <r>`
- (act) `nabi cli notify --title <text> [--body <text>]` — desktop/toast notification.
- (act) `nabi cli open-browser [--path <dir>]` — open the file browser.
- (act) `nabi cli web-back` — go back in the built-in browser tab.
- (act) `nabi cli web-forward` — go forward.
- (act) `nabi cli web-reload` — reload the page.
- (act) `nabi cli web-stop` — stop loading.
- (act) `nabi cli web-list` — list the open web tabs (id and current address).
- (inject) `nabi cli restart`
  Close and reopen nabiTerm, without asking. The workspace is saved first, so tabs, splits and
  panes come back as they were. Use it after changing something that only takes effect on a
  fresh start. **Every pane dies, including yours** — the new instance is a new process.
- (inject) `nabi cli quit`
  Close nabiTerm **without asking**. Closing the window normally pops a "really close?" dialog
  when more than one tab is open — fine for a person, but it stops a script dead. This saves
  the workspace first, so everything comes back next launch; it is not a kill.
  **Every pane dies, including yours.** Say what will happen before you call it.
- (inject) `nabi cli update [--check]`
  Upgrade nabiTerm to the latest release, with no clicking. It checks GitHub, downloads the
  installer, **verifies its SHA-256 against the one the release published**, installs silently
  and restarts nabiTerm. `--check` only reports whether a newer version exists.
  Two things to know before you call it: nabiTerm restarts, so **every pane dies — including
  yours**; and installing counts as Inject (see Permissions), so in `ask` mode a person has to
  allow it once. If you are running inside a pane, tell the user what will happen first.

### Typing into panes, moving files, one-way doors

- (inject) `nabi cli send --pane <id> --data <text> [--raw]`
- (inject) `nabi cli send --pane <id> --keys "ctrl+c enter esc pgup f1"` — named keys, no escape codes needed.
  Type into a pane. Append a carriage return to press Enter (PowerShell: "cmd`r").
  Default wraps as a bracketed paste; `--raw` sends bytes verbatim (control keys).
- (inject) `nabi cli kill --pane <id>` — close a pane.
- (inject) `nabi cli open-sftp --session <name>` — open an SFTP browser for a saved session.
- (inject) `nabi cli sftp-get --remote <path> --local <path>` — download one file over the
  currently open SFTP connection (waits for completion; shows in the transfer queue).
- (inject) `nabi cli sftp-put --local <path> --remote <path>` — upload one file (same rules).

### Remote files (needs an SFTP tab already connected)

- (act) `nabi cli sftp-list [--path <remote dir>]` — list a remote directory as JSON
  (`name`, `is_dir`, `size`, `mode`, `mtime`). Fails if no SFTP connection is open.

## Targeting a pane without an id

Instead of `--pane <id>` you can match by property:

    nabi cli capture --match "title:build,state:idle"
    # keys: title (substring), cwd (prefix), kind, state, id
    # prefix a value with ! to negate, e.g. --match "kind:ssh,state:!idle"

Matching more than one pane is refused by default — sending into the wrong pane is worse
than being asked. Add `--all` when you mean every match, and the command runs once per pane:

    nabi cli send --match "kind:ssh,state:idle" --all --data "uptime\r"

It fails if any pane failed, so a partial result never reports success.

## Permissions

Control mode lives in nabiTerm ▸ Settings ▸ Behavior ▸ Agent control: off / ask / on.

- off  — all control is refused.
- ask  — (default) `(read)` verbs always work. The first `(act)` and the first `(inject)`
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

## Recipes

**Run a build in its own pane and report back.**

    $p = (nabi cli spawn --dock split-right --cwd C:\proj --json | ConvertFrom-Json).pane
    nabi cli send --pane $p --data "cargo build`r"
    nabi cli wait --pane $p --until command-done --timeout 900000
    nabi cli capture --pane $p --lines 80        # read the tail, not the whole thing

**Check a claim about the UI instead of guessing.** Text output cannot tell you whether an
image drew, a colour is right, or a layout broke. Pixels can.

    nabi cli screenshot --out C:	emp
ow.png
    # then read that PNG with your own image-reading tool

**Watch for one thing without polling.**

    nabi cli wait --pane 3 --until output --regex "error\[E\d+\]" --timeout 120000

**Work with a remote machine.** Open the SSH session the user already saved, then use the
SFTP verbs — they run over that connection, so you never handle credentials.

    nabi cli open-sftp --session prod
    nabi cli sftp-list --path /var/log
    nabi cli sftp-get --remote /var/log/app.log --local C:	emppp.log
    nabi cli open-file --path C:	emppp.log       # let the user read it properly

**Read a web page without curl.** The built-in browser renders real pages (JS included),
so you see what a person sees — and you can act on it.

    nabi cli web --url https://github.com/matrixism-cmyk/NabiTerm/releases
    nabi cli web-eval --js "document.body.innerText.slice(0, 3000)"

**Ask another agent to do something and wait for it.**

    nabi cli agent prompt --pane 5 --data "run the tests and report" --wait --until idle

**Say what you are doing, all the time.** The user sees this in the status bar and on the
tab, and it is the difference between waiting patiently and asking you every two minutes.

    nabi cli status set task "refactoring auth"
    nabi cli progress --pct 40
    nabi cli progress                                # clear it when done

## When something fails

Errors are plain text on stderr with a non-zero exit code. The ones you will actually hit:

- **`approval pending`** — the control mode is `ask` and this is the first Act or Inject.
  Tell the user to click Allow in nabiTerm, then run the same command again. Do not retry
  in a loop; nothing changes until a person clicks.
- **`control disabled`** — the mode is `off`. Only the user can change it, in
  Settings ▸ Behavior ▸ Agent control. Say so and stop.
- **`pane <n> not found`** — that pane closed. Run `nabi cli list` and pick again; never
  assume an id you saw earlier is still alive.
- **`pipe not found` / connection refused** — nabiTerm is not running, or you are in a shell
  it did not start (so `NABI_CONTROL_PIPE` is unset). Nothing here will work; say so.
- **`no SFTP connection`** — the sftp-* verbs need a connected SFTP tab. `open-sftp` first.
- **shell not installed** — `spawn` names the missing executable (e.g. `pwsh.exe`) instead of
  hanging. Fall back to `--shell powershell`, which is always present on Windows.
- **`wait` timed out** — it returns non-zero and tells you. That is information, not a bug:
  the command is still running. `capture` the pane to see where it got stuck.

## Rules to keep

- **Send ASCII only with `send` and `agent prompt`.** Terminal programs — including other
  AI agents' text UIs — corrupt their own screen when non-ASCII text is injected. Write to a
  file and open it instead if the content must be Korean, Japanese or emoji.
- **Read the tail, not the whole scrollback.** `capture --lines 100` almost always answers
  the question. Dumping tens of thousands of lines wastes your context and tells you less.
- **One pane, one job.** Spawn a pane per task rather than interleaving commands in one; you
  can then `wait` on each independently and the user can see which is which.
- **Clean up what you spawned.** `nabi cli kill --pane <id>` when a pane's job is done.
  Panes you leave behind are the user's to close.
- **Never leave an unbounded polling loop running.** Use `wait`, which blocks properly and
  times out. A `while true` loop in a spawned shell keeps running after you stop.
- **The user's own pane is not yours.** Do not `send` into the pane a person is typing in.

## Typical workflow (PowerShell)

    $pane = (nabi cli spawn --shell powershell --cwd C:\proj --dock split-right --json | ConvertFrom-Json).pane
    nabi cli send --pane $pane --data "cargo test`r"
    nabi cli wait --pane $pane --until command-done --timeout 600000
    nabi cli capture --pane $pane --lines 200
    nabi cli notify --title "Tests finished"

Notes: `nabi` is the same exe that runs the GUI; the `cli` subcommand does one
request and exits. Pane ids are integers from `nabi cli list`.
"#;

