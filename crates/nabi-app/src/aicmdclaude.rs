//! Claude Code(`claude`)의 슬래시 명령 표 — 2026-08-21 공식 문서 전수 반영.
//!
//! 왜 별도 파일인가: 명령이 80개를 넘어 aicmdcmds.rs가 한 파일에 담기 어렵다.
//! 목록은 `code.claude.com/docs/en/commands`(전체 표)를 받아 적고, 실제 설치본
//! (claude 2.1.236) 바이너리의 명령 레지스트리(`name:"…"`)와 대조해 **존재하는 것만**
//! 남겼다. 별칭(/cost=/usage, /settings=/config, /share=/bug, /review=/code-review,
//! /update=/upgrade)과 폐지된 /pr-comments는 중복이라 뺐다.
//!
//! 바 버튼(primary)은 자주 쓰는 7개만, 나머지는 "⋯" 안에서 **주제별 하위 메뉴**로 묶는다
//! (메뉴 비대화 방지 원칙). 표기는 앱 전체 규칙을 따른다 — **한국어 요약명을 보여주고**
//! 실제 슬래시 명령과 설명은 툴팁에 둔다(사용자 지적 2026-08-22: 새로 넣은 명령만
//! `/cmd`로 나와 기존 명령들과 어긋나 있었다).

use crate::aicmdcmds::{c, u, BarCmd, CmdGroup};

/// 바에 바로 노출할 주요 명령.
pub(crate) fn primary() -> &'static [BarCmd] {
    static A: &[BarCmd] = &[
        c("/compact", "aicb.l.compact", "aicb.claude.compact"),
        c("/clear", "aicb.l.clear", "aicb.claude.clear"),
        u("/context", "aicb.l.context", "aicb.claude.context"),
        // 별칭·단계는 공식 CLI 레퍼런스 기준(fable/opus/sonnet/haiku, low~ultracode).
        // 첫 항목은 CLI의 대화식 선택창.
        BarCmd { cmd: "/model", label: "aicb.l.model", desc: "aicb.claude.model", sub: &[
            ("/model", "/model"), ("fable", "/model fable"), ("opus", "/model opus"),
            ("sonnet", "/model sonnet"), ("haiku", "/model haiku"),
        ], opens_ui: true },
        BarCmd { cmd: "/effort", label: "aicb.l.effort", desc: "aicb.claude.effort", sub: &[
            ("/effort", "/effort"), ("low", "/effort low"), ("medium", "/effort medium"),
            ("high", "/effort high"), ("xhigh", "/effort xhigh"), ("max", "/effort max"),
            ("ultracode", "/effort ultracode"),
        ], opens_ui: true },
        u("/resume", "aicb.l.resume", "aicb.claude.resume"),
        u("/usage", "aicb.l.usage", "aicb.claude.usage"),
    ];
    A
}

/// "⋯" 더보기 — 주제별 묶음.
pub(crate) fn groups() -> &'static [CmdGroup] {
    static G: &[CmdGroup] = &[
        CmdGroup { label: "aicb.g.session", cmds: SESSION },
        CmdGroup { label: "aicb.g.review", cmds: REVIEW },
        CmdGroup { label: "aicb.g.work", cmds: WORK },
        CmdGroup { label: "aicb.g.project", cmds: PROJECT },
        CmdGroup { label: "aicb.g.ext", cmds: EXT },
        CmdGroup { label: "aicb.g.pref", cmds: PREF },
        CmdGroup { label: "aicb.g.account", cmds: ACCOUNT },
        CmdGroup { label: "aicb.g.help", cmds: HELP },
    ];
    G
}

/// 세션·기록 — 대화를 되돌리고 꺼내고 정리한다.
static SESSION: &[BarCmd] = &[
    u("/rewind", "aicb.l.rewind", "aicb.claude.rewind"),
    c("/recap", "aicb.l.recap", "aicb.claude.recap"),
    u("/export", "aicb.l.export", "aicb.claude.export"),
    c("/copy", "aicb.l.copy", "aicb.claude.copy"),
    u("/import", "aicb.l.import", "aicb.claude.import"),
    c("/autocompact", "aicb.l.autocompact", "aicb.claude.autocompact"),
    u("/status", "aicb.l.status", "aicb.claude.status"),
    u("/todos", "aicb.l.todos", "aicb.claude.todos"),
    c("/exit", "aicb.l.exit", "aicb.claude.exit"),
];

/// 코드 검토 — 지금 diff를 여러 각도로 본다.
static REVIEW: &[BarCmd] = &[
    u("/code-review", "aicb.l.codereview", "aicb.claude.codereview"),
    u("/security-review", "aicb.l.secreview", "aicb.claude.secreview"),
    u("/simplify", "aicb.l.simplify", "aicb.claude.simplify"),
    u("/verify", "aicb.l.verify", "aicb.claude.verify"),
    u("/diff", "aicb.l.diff", "aicb.claude.diff"),
    u("/autofix-pr", "aicb.l.autofixpr", "aicb.claude.autofixpr"),
];

/// 계획·작업 — 무엇을 어떻게 시킬지 정한다.
static WORK: &[BarCmd] = &[
    c("/plan", "aicb.l.plan", "aicb.claude.plan"),
    c("/goal", "aicb.l.goal", "aicb.claude.goal"),
    c("/loop", "aicb.l.loop", "aicb.claude.loop"),
    u("/tasks", "aicb.l.tasks", "aicb.claude.tasks"),
    c("/background", "aicb.l.background", "aicb.claude.background"),
    u("/batch", "aicb.l.batch", "aicb.claude.batch"),
    c("/subtask", "aicb.l.subtask", "aicb.claude.subtask"),
    c("/fork", "aicb.l.fork", "aicb.claude.fork"),
    c("/branch", "aicb.l.branch", "aicb.claude.branch"),
    u("/agents", "aicb.l.agents", "aicb.claude.agents"),
    c("/list-agents", "aicb.l.listagents", "aicb.claude.listagents"),
    u("/focus", "aicb.l.focus", "aicb.claude.focus"),
    c("/deep-research", "aicb.l.deepresearch", "aicb.claude.deepresearch"),
    c("/dataviz", "aicb.l.dataviz", "aicb.claude.dataviz"),
];

/// 프로젝트·파일 — 작업 범위와 기억을 다룬다.
static PROJECT: &[BarCmd] = &[
    c("/init", "aicb.l.init", "aicb.claude.init"),
    u("/add-dir", "aicb.l.adddir", "aicb.claude.adddir"),
    u("/cd", "aicb.l.cd", "aicb.claude.cd"),
    u("/memory", "aicb.l.memory", "aicb.claude.memory"),
    u("/rename", "aicb.l.renamefile", "aicb.claude.renamefile"),
    u("/worktree", "aicb.l.worktree", "aicb.claude.worktree"),
    c("/tools", "aicb.l.toolsdoc", "aicb.claude.tools"),
];

/// 확장·연동 — MCP·플러그인·스킬·외부 앱.
static EXT: &[BarCmd] = &[
    u("/mcp", "aicb.l.mcp", "aicb.claude.mcp"),
    u("/plugin", "aicb.l.plugin", "aicb.claude.plugin"),
    c("/reload-plugins", "aicb.l.reloadplugins", "aicb.claude.reloadplugins"),
    u("/skills", "aicb.l.skills", "aicb.claude.skills"),
    u("/hooks", "aicb.l.hooks", "aicb.claude.hooks"),
    u("/ide", "aicb.l.ide", "aicb.claude.ide"),
    u("/chrome", "aicb.l.chrome", "aicb.claude.chrome"),
    u("/install-github-app", "aicb.l.ghapp", "aicb.claude.ghapp"),
    u("/install-slack-app", "aicb.l.slackapp", "aicb.claude.slackapp"),
    u("/claude-api", "aicb.l.claudeapi", "aicb.claude.claudeapi"),
    c("/design-sync", "aicb.l.designsync", "aicb.claude.designsync"),
    u("/design-login", "aicb.l.designlogin", "aicb.claude.designlogin"),
];

/// 설정·모양 — 권한·단축키·테마.
static PREF: &[BarCmd] = &[
    u("/config", "aicb.l.settings", "aicb.claude.config"),
    u("/permissions", "aicb.l.permissions", "aicb.claude.permissions"),
    u("/fewer-permission-prompts", "aicb.l.fewerperms", "aicb.claude.fewerperms"),
    u("/keybindings", "aicb.l.keybindings", "aicb.claude.keybindings"),
    u("/theme", "aicb.l.theme", "aicb.claude.theme"),
    u("/color", "aicb.l.color", "aicb.claude.color"),
    c("/vim", "aicb.l.vim", "aicb.claude.vim"),
    c("/fast", "aicb.l.fast", "aicb.claude.fast"),
    u("/web", "aicb.l.web", "aicb.claude.web"),
    u("/privacy-settings", "aicb.l.privacy", "aicb.claude.privacy"),
];

/// 계정·기기 — 로그인·플랜·다른 기기로 잇기.
static ACCOUNT: &[BarCmd] = &[
    u("/login", "aicb.l.login", "aicb.claude.login"),
    c("/logout", "aicb.l.logout", "aicb.claude.logout"),
    c("/upgrade", "aicb.l.upgrade", "aicb.claude.upgrade"),
    u("/passes", "aicb.l.passes", "aicb.claude.passes"),
    u("/insights", "aicb.l.insights", "aicb.claude.insights"),
    u("/remote-control", "aicb.l.remotectl", "aicb.claude.remotectl"),
    u("/teleport", "aicb.l.teleport", "aicb.claude.teleport"),
    u("/desktop", "aicb.l.desktop", "aicb.claude.desktop"),
    u("/mobile", "aicb.l.mobile", "aicb.claude.mobile"),
];

/// 도움·진단 — 막혔을 때.
static HELP: &[BarCmd] = &[
    u("/help", "aicb.l.help", "aicb.help"),
    u("/doctor", "aicb.l.doctor", "aicb.claude.doctor"),
    u("/release-notes", "aicb.l.relnotes", "aicb.claude.relnotes"),
    u("/feedback", "aicb.l.feedback", "aicb.claude.feedback"),
    u("/bug", "aicb.l.bug", "aicb.claude.bug"),
    u("/debug", "aicb.l.debug", "aicb.claude.debug"),
    c("/heapdump", "aicb.l.heapdump", "aicb.claude.heapdump"),
    u("/advisor", "aicb.l.advisor", "aicb.claude.advisor"),
    c("/btw", "aicb.l.btw", "aicb.claude.btw"),
    u("/powerup", "aicb.l.powerup", "aicb.claude.powerup"),
    u("/radio", "aicb.l.radio", "aicb.claude.radio"),
];
