//! Claude 외 CLI의 슬래시 명령 표 — codex · agy(Antigravity) · aider.
//!
//! 사용자 지적(2026-08-22): Claude만 최신이고 나머지는 몇 개뿐이라 바가 빈약했다.
//! 셋 다 다시 조사했다.
//! - **codex**: 설치본 `@openai/codex 0.147.0`의 네이티브 바이너리에서 명령 이름·설명
//!   블록을 직접 뽑아 냈고, 공식 문서(learn.chatgpt.com/docs/developer-commands)와
//!   시작 화면 힌트로 교차 확인했다. 확인되지 않은 디버그·장식용 명령은 **일부러 뺐다**
//!   (없는 명령을 넣으면 눌렀을 때 그대로 셸에 새 문자열이 찍힌다).
//! - **agy**: antigravity.google/docs/cli/reference 전수.
//! - **aider**: aider.chat/docs/usage/commands.html 전수(별칭 /edit·/ok·/quit는 제외).
//!
//! 표기 규칙은 Claude와 같다 — 한국어 요약명을 보여주고 실제 명령·설명은 툴팁에.

use crate::aicmdcmds::{c, u, BarCmd, CmdGroup};

// ─── codex ───────────────────────────────────────────────────────────────────

pub(crate) fn codex_primary() -> &'static [BarCmd] {
    static A: &[BarCmd] = &[
        c("/compact", "aicb.l.compact", "aicb.codex.compact"),
        c("/clear", "aicb.l.clear", "aicb.codex.clear"),
        u("/permissions", "aicb.l.permissions", "aicb.codex.permissions"),
        u("/diff", "aicb.l.diff", "aicb.codex.diff"),
        BarCmd { cmd: "/model", label: "aicb.l.model", desc: "aicb.codex.model", sub: &[], opens_ui: true },
        u("/status", "aicb.l.status", "aicb.codex.status"),
    ];
    A
}

pub(crate) fn codex_groups() -> &'static [CmdGroup] {
    static G: &[CmdGroup] = &[
        CmdGroup { label: "aicb.g.session", cmds: CX_SESSION },
        CmdGroup { label: "aicb.g.review", cmds: CX_REVIEW },
        CmdGroup { label: "aicb.g.work", cmds: CX_WORK },
        CmdGroup { label: "aicb.g.project", cmds: CX_PROJECT },
        CmdGroup { label: "aicb.g.ext", cmds: CX_EXT },
        CmdGroup { label: "aicb.g.pref", cmds: CX_PREF },
        CmdGroup { label: "aicb.g.account", cmds: CX_ACCOUNT },
    ];
    G
}

static CX_SESSION: &[BarCmd] = &[
    c("/new", "aicb.l.newchat", "aicb.codex.new"),
    u("/resume", "aicb.l.resume", "aicb.codex.resume"),
    c("/fork", "aicb.l.fork", "aicb.codex.fork"),
    u("/rename", "aicb.l.rename", "aicb.codex.rename"),
    c("/archive", "aicb.l.archive", "aicb.codex.archive"),
    c("/delete", "aicb.l.delsession", "aicb.codex.delete"),
    c("/copy", "aicb.l.copy", "aicb.codex.copy"),
    u("/usage", "aicb.l.usage", "aicb.codex.usage"),
];

static CX_REVIEW: &[BarCmd] = &[
    u("/review", "aicb.l.codereview", "aicb.codex.review"),
    u("/mention", "aicb.l.mention", "aicb.codex.mention"),
];

static CX_WORK: &[BarCmd] = &[
    c("/goal", "aicb.l.goal", "aicb.codex.goal"),
    c("/btw", "aicb.l.btw", "aicb.codex.btw"),
    u("/subagents", "aicb.l.subagents", "aicb.codex.subagents"),
    u("/ps", "aicb.l.bglist", "aicb.codex.ps"),
    c("/stop", "aicb.l.bgstop", "aicb.codex.stop"),
];

static CX_PROJECT: &[BarCmd] = &[
    c("/init", "aicb.l.init", "aicb.codex.init"),
    u("/import", "aicb.l.importsetup", "aicb.codex.import"),
];

static CX_EXT: &[BarCmd] = &[
    u("/mcp", "aicb.l.mcp", "aicb.codex.mcp"),
    u("/apps", "aicb.l.apps", "aicb.codex.apps"),
    u("/plugins", "aicb.l.plugin", "aicb.codex.plugins"),
    u("/skills", "aicb.l.skills", "aicb.codex.skills"),
    u("/hooks", "aicb.l.hooks", "aicb.codex.hooks"),
    u("/ide", "aicb.l.ide", "aicb.codex.ide"),
];

static CX_PREF: &[BarCmd] = &[
    u("/approve", "aicb.l.approve", "aicb.codex.approve"),
    u("/keymap", "aicb.l.keybindings", "aicb.codex.keymap"),
    c("/vim", "aicb.l.vim", "aicb.codex.vim"),
    u("/experimental", "aicb.l.experimental", "aicb.codex.experimental"),
    u("/memories", "aicb.l.memory", "aicb.codex.memories"),
    // Windows 전용 — 우리 사용자는 전부 Windows라 그대로 노출한다.
    u("/setup-default-sandbox", "aicb.l.sandboxsetup", "aicb.codex.sandboxsetup"),
    u("/sandbox-add-read-dir", "aicb.l.sandboxread", "aicb.codex.sandboxread"),
];

static CX_ACCOUNT: &[BarCmd] = &[
    c("/logout", "aicb.l.logout", "aicb.codex.logout"),
    u("/feedback", "aicb.l.feedback", "aicb.codex.feedback"),
];

// ─── agy (Antigravity CLI) ───────────────────────────────────────────────────

pub(crate) fn agy_primary() -> &'static [BarCmd] {
    static A: &[BarCmd] = &[
        c("/clear", "aicb.l.clear", "aicb.agy.clear"),
        u("/context", "aicb.l.context", "aicb.agy.context"),
        u("/usage", "aicb.l.usage", "aicb.agy.usage"),
        u("/model", "aicb.l.model", "aicb.agy.model"),
        u("/resume", "aicb.l.resume", "aicb.agy.resume"),
        u("/diff", "aicb.l.diff", "aicb.agy.diff"),
    ];
    A
}

pub(crate) fn agy_groups() -> &'static [CmdGroup] {
    static G: &[CmdGroup] = &[
        CmdGroup { label: "aicb.g.session", cmds: AG_SESSION },
        CmdGroup { label: "aicb.g.work", cmds: AG_WORK },
        CmdGroup { label: "aicb.g.project", cmds: AG_PROJECT },
        CmdGroup { label: "aicb.g.ext", cmds: AG_EXT },
        CmdGroup { label: "aicb.g.pref", cmds: AG_PREF },
        CmdGroup { label: "aicb.g.account", cmds: AG_ACCOUNT },
        CmdGroup { label: "aicb.g.help", cmds: AG_HELP },
    ];
    G
}

static AG_SESSION: &[BarCmd] = &[
    c("/copy", "aicb.l.copy", "aicb.agy.copy"),
    u("/rewind", "aicb.l.rewind", "aicb.agy.rewind"),
    u("/rename", "aicb.l.rename", "aicb.agy.rename"),
    c("/fork", "aicb.l.fork", "aicb.agy.fork"),
];

static AG_WORK: &[BarCmd] = &[
    u("/agents", "aicb.l.agents", "aicb.agy.agents"),
    u("/tasks", "aicb.l.tasks", "aicb.agy.tasks"),
    c("/planning", "aicb.l.plan", "aicb.agy.planning"),
    c("/fast", "aicb.l.fast", "aicb.agy.fast"),
    c("/btw", "aicb.l.btw", "aicb.agy.btw"),
];

static AG_PROJECT: &[BarCmd] = &[
    u("/add-dir", "aicb.l.adddir", "aicb.agy.adddir"),
    u("/open", "aicb.l.openinapp", "aicb.agy.open"),
    u("/artifact", "aicb.l.artifact", "aicb.agy.artifact"),
];

static AG_EXT: &[BarCmd] = &[
    u("/mcp", "aicb.l.mcp", "aicb.agy.mcp"),
    u("/skills", "aicb.l.skills", "aicb.agy.skills"),
    u("/hooks", "aicb.l.hooks", "aicb.agy.hooks"),
];

static AG_PREF: &[BarCmd] = &[
    u("/config", "aicb.l.settings", "aicb.agy.config"),
    u("/permissions", "aicb.l.perms", "aicb.agy.perms"),
    u("/keybindings", "aicb.l.keybindings", "aicb.agy.keybindings"),
    u("/statusline", "aicb.l.statusline", "aicb.agy.statusline"),
    c("/title", "aicb.l.wintitle", "aicb.agy.title"),
];

static AG_ACCOUNT: &[BarCmd] = &[
    u("/credits", "aicb.l.credits", "aicb.agy.credits"),
    c("/logout", "aicb.l.logout", "aicb.agy.logout"),
];

static AG_HELP: &[BarCmd] = &[
    u("/help", "aicb.l.help", "aicb.help"),
    u("/feedback", "aicb.l.feedback", "aicb.agy.feedback"),
];

// ─── aider ───────────────────────────────────────────────────────────────────

pub(crate) fn aider_primary() -> &'static [BarCmd] {
    static A: &[BarCmd] = &[
        c("/undo", "aicb.l.undo", "aicb.aider.undo"),
        u("/diff", "aicb.l.diff", "aicb.aider.diff"),
        c("/commit", "aicb.l.commit", "aicb.aider.commit"),
        c("/clear", "aicb.l.clear", "aicb.aider.clear"),
        c("/tokens", "aicb.l.tokens", "aicb.aider.tokens"),
        BarCmd { cmd: "/model", label: "aicb.l.model", desc: "aicb.aider.model", sub: &[], opens_ui: true },
    ];
    A
}

pub(crate) fn aider_groups() -> &'static [CmdGroup] {
    static G: &[CmdGroup] = &[
        CmdGroup { label: "aicb.g.files", cmds: AD_FILES },
        CmdGroup { label: "aicb.g.mode", cmds: AD_MODE },
        CmdGroup { label: "aicb.g.run", cmds: AD_RUN },
        CmdGroup { label: "aicb.g.models", cmds: AD_MODELS },
        CmdGroup { label: "aicb.g.session", cmds: AD_SESSION },
    ];
    G
}

static AD_FILES: &[BarCmd] = &[
    u("/add", "aicb.l.addfiles", "aicb.aider.add"),
    c("/drop", "aicb.l.drop", "aicb.aider.drop"),
    u("/read-only", "aicb.l.readonly", "aicb.aider.readonly"),
    c("/ls", "aicb.l.filelist", "aicb.aider.ls"),
    c("/map", "aicb.l.map", "aicb.aider.map"),
    c("/map-refresh", "aicb.l.maprefresh", "aicb.aider.maprefresh"),
    c("/reset", "aicb.l.resetchat", "aicb.aider.reset"),
    c("/paste", "aicb.l.paste", "aicb.aider.paste"),
];

static AD_MODE: &[BarCmd] = &[
    c("/code", "aicb.l.codemode", "aicb.aider.code"),
    c("/ask", "aicb.l.askmode", "aicb.aider.ask"),
    c("/architect", "aicb.l.architect", "aicb.aider.architect"),
    u("/chat-mode", "aicb.l.chatmode", "aicb.aider.chatmode"),
    c("/context", "aicb.l.context", "aicb.aider.context"),
    u("/editor", "aicb.l.editor", "aicb.aider.editor"),
    c("/multiline-mode", "aicb.l.multiline", "aicb.aider.multiline"),
];

static AD_RUN: &[BarCmd] = &[
    u("/run", "aicb.l.runshell", "aicb.aider.run"),
    c("/test", "aicb.l.test", "aicb.aider.test"),
    c("/lint", "aicb.l.lint", "aicb.aider.lint"),
    u("/git", "aicb.l.gitcmd", "aicb.aider.git"),
    u("/web", "aicb.l.scrapeweb", "aicb.aider.web"),
];

static AD_MODELS: &[BarCmd] = &[
    u("/models", "aicb.l.modelsearch", "aicb.aider.models"),
    u("/editor-model", "aicb.l.editormodel", "aicb.aider.editormodel"),
    u("/weak-model", "aicb.l.weakmodel", "aicb.aider.weakmodel"),
    u("/reasoning-effort", "aicb.l.effort", "aicb.aider.effort"),
    u("/think-tokens", "aicb.l.thinktokens", "aicb.aider.thinktokens"),
];

static AD_SESSION: &[BarCmd] = &[
    c("/copy", "aicb.l.copy", "aicb.aider.copy"),
    c("/copy-context", "aicb.l.copyctx", "aicb.aider.copyctx"),
    u("/save", "aicb.l.savechat", "aicb.aider.save"),
    u("/load", "aicb.l.loadchat", "aicb.aider.load"),
    u("/settings", "aicb.l.settings", "aicb.aider.settings"),
    u("/report", "aicb.l.report", "aicb.aider.report"),
    c("/voice", "aicb.l.voice", "aicb.aider.voice"),
    u("/help", "aicb.l.help", "aicb.help"),
];
