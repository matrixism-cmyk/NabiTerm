//! Claude Code(`claude`)의 슬래시 명령 표 — 2026-08-21 공식 문서 전수 반영.
//!
//! 왜 별도 파일인가: 명령이 80개를 넘어 aicmdcmds.rs가 한 파일에 담기 어렵다.
//! 목록은 `code.claude.com/docs/en/commands`(전체 표)를 받아 적고, 실제 설치본
//! (claude 2.1.236) 바이너리의 명령 레지스트리(`name:"…"`)와 대조해 **존재하는 것만**
//! 남겼다. 별칭(/cost=/usage, /settings=/config, /share=/bug, /review=/code-review,
//! /update=/upgrade)과 폐지된 /pr-comments는 중복이라 뺐다.
//!
//! 바 버튼(primary)은 자주 쓰는 7개만, 나머지는 "⋯" 안에서 **주제별 하위 메뉴**로 묶는다
//! (메뉴 비대화 방지 원칙). ⋯ 안에서는 라벨 대신 **실제 슬래시 명령**을 보여주고 한국어
//! 설명은 툴팁에 둔다 — 80개에 억지 한국어 이름을 붙이면 오히려 못 찾는다.

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
    u("/rewind", "", "aicb.claude.rewind"),
    c("/recap", "", "aicb.claude.recap"),
    u("/export", "", "aicb.claude.export"),
    c("/copy", "", "aicb.claude.copy"),
    u("/import", "", "aicb.claude.import"),
    c("/autocompact", "", "aicb.claude.autocompact"),
    u("/status", "", "aicb.claude.status"),
    u("/todos", "", "aicb.claude.todos"),
    c("/exit", "", "aicb.claude.exit"),
];

/// 코드 검토 — 지금 diff를 여러 각도로 본다.
static REVIEW: &[BarCmd] = &[
    u("/code-review", "", "aicb.claude.codereview"),
    u("/security-review", "", "aicb.claude.secreview"),
    u("/simplify", "", "aicb.claude.simplify"),
    u("/verify", "", "aicb.claude.verify"),
    u("/diff", "", "aicb.claude.diff"),
    u("/autofix-pr", "", "aicb.claude.autofixpr"),
];

/// 계획·작업 — 무엇을 어떻게 시킬지 정한다.
static WORK: &[BarCmd] = &[
    c("/plan", "", "aicb.claude.plan"),
    c("/goal", "", "aicb.claude.goal"),
    c("/loop", "", "aicb.claude.loop"),
    u("/tasks", "", "aicb.claude.tasks"),
    c("/background", "", "aicb.claude.background"),
    u("/batch", "", "aicb.claude.batch"),
    c("/subtask", "", "aicb.claude.subtask"),
    c("/fork", "", "aicb.claude.fork"),
    c("/branch", "", "aicb.claude.branch"),
    u("/agents", "", "aicb.claude.agents"),
    c("/list-agents", "", "aicb.claude.listagents"),
    u("/focus", "", "aicb.claude.focus"),
    c("/deep-research", "", "aicb.claude.deepresearch"),
    c("/dataviz", "", "aicb.claude.dataviz"),
];

/// 프로젝트·파일 — 작업 범위와 기억을 다룬다.
static PROJECT: &[BarCmd] = &[
    c("/init", "", "aicb.claude.init"),
    u("/add-dir", "", "aicb.claude.adddir"),
    u("/cd", "", "aicb.claude.cd"),
    u("/memory", "", "aicb.claude.memory"),
    u("/rename", "", "aicb.claude.renamefile"),
    u("/worktree", "", "aicb.claude.worktree"),
    c("/tools", "", "aicb.claude.tools"),
];

/// 확장·연동 — MCP·플러그인·스킬·외부 앱.
static EXT: &[BarCmd] = &[
    u("/mcp", "", "aicb.claude.mcp"),
    u("/plugin", "", "aicb.claude.plugin"),
    c("/reload-plugins", "", "aicb.claude.reloadplugins"),
    u("/skills", "", "aicb.claude.skills"),
    u("/hooks", "", "aicb.claude.hooks"),
    u("/ide", "", "aicb.claude.ide"),
    u("/chrome", "", "aicb.claude.chrome"),
    u("/install-github-app", "", "aicb.claude.ghapp"),
    u("/install-slack-app", "", "aicb.claude.slackapp"),
    u("/claude-api", "", "aicb.claude.claudeapi"),
    c("/design-sync", "", "aicb.claude.designsync"),
    u("/design-login", "", "aicb.claude.designlogin"),
];

/// 설정·모양 — 권한·단축키·테마.
static PREF: &[BarCmd] = &[
    u("/config", "", "aicb.claude.config"),
    u("/permissions", "", "aicb.claude.permissions"),
    u("/fewer-permission-prompts", "", "aicb.claude.fewerperms"),
    u("/keybindings", "", "aicb.claude.keybindings"),
    u("/theme", "", "aicb.claude.theme"),
    u("/color", "", "aicb.claude.color"),
    c("/vim", "", "aicb.claude.vim"),
    c("/fast", "", "aicb.claude.fast"),
    u("/web", "", "aicb.claude.web"),
    u("/privacy-settings", "", "aicb.claude.privacy"),
];

/// 계정·기기 — 로그인·플랜·다른 기기로 잇기.
static ACCOUNT: &[BarCmd] = &[
    u("/login", "", "aicb.claude.login"),
    c("/logout", "", "aicb.claude.logout"),
    c("/upgrade", "", "aicb.claude.upgrade"),
    u("/passes", "", "aicb.claude.passes"),
    u("/insights", "", "aicb.claude.insights"),
    u("/remote-control", "", "aicb.claude.remotectl"),
    u("/teleport", "", "aicb.claude.teleport"),
    u("/desktop", "", "aicb.claude.desktop"),
    u("/mobile", "", "aicb.claude.mobile"),
];

/// 도움·진단 — 막혔을 때.
static HELP: &[BarCmd] = &[
    u("/help", "", "aicb.help"),
    u("/doctor", "", "aicb.claude.doctor"),
    u("/release-notes", "", "aicb.claude.relnotes"),
    u("/feedback", "", "aicb.claude.feedback"),
    u("/bug", "", "aicb.claude.bug"),
    u("/debug", "", "aicb.claude.debug"),
    c("/heapdump", "", "aicb.claude.heapdump"),
    u("/advisor", "", "aicb.claude.advisor"),
    c("/btw", "", "aicb.claude.btw"),
    u("/powerup", "", "aicb.claude.powerup"),
    u("/radio", "", "aicb.claude.radio"),
];
