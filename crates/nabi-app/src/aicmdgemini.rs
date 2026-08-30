//! Gemini CLI 의 슬래시 명령 표.
//!
//! ## 왜 이제야 붙이는가
//!
//! 명령 이름과 설명은 **이미 번역까지 되어 있었다**(16개). 그런데 표를 만들어 붙이는 일을
//! 안 해서, 사용자가 `gemini` 를 띄워도 명령 바가 아무것도 모르는 상태였다. 카탈로그에
//! 있는데 아무도 안 쓰는 키를 세다가 드러났다(배치 BH).
//!
//! ## 무엇을 넣었나
//!
//! 다른 CLI 와 같은 규칙이다 — **확인된 명령만 넣는다.** 없는 명령을 넣으면 눌렀을 때
//! 그 글자가 그대로 셸에 찍히고, 사용자는 우리가 고장 났다고 본다. 그래서 번역이 준비된
//! 열여섯 개만 담고, 그 밖의 것은 넣지 않았다.
//!
//! 표기는 다른 표와 같다 — 한국어 요약명을 보여 주고 실제 명령과 설명은 툴팁에 둔다.

use crate::aicmdcmds::{c, u, BarCmd, CmdGroup};

/// 바에 바로 보이는 것 — 가장 자주 쓰는 여섯.
pub(crate) fn primary() -> &'static [BarCmd] {
    static A: &[BarCmd] = &[
        c("/compress", "aicb.l.compact", "aicb.gemini.compress"),
        c("/clear", "aicb.l.clear", "aicb.gemini.clear"),
        u("/stats", "aicb.l.tokens", "aicb.gemini.stats"),
        u("/tools", "aicb.l.tools", "aicb.gemini.tools"),
        u("/memory", "aicb.l.memory", "aicb.gemini.memory"),
        u("/chat", "aicb.l.chat", "aicb.gemini.chat"),
    ];
    A
}

/// 더보기(⋯) — 주제별 묶음.
pub(crate) fn groups() -> &'static [CmdGroup] {
    static G: &[CmdGroup] = &[
        CmdGroup { label: "aicb.g.session", cmds: SESSION },
        CmdGroup { label: "aicb.g.project", cmds: PROJECT },
        CmdGroup { label: "aicb.g.ext", cmds: EXT },
        CmdGroup { label: "aicb.g.pref", cmds: PREF },
    ];
    G
}

static SESSION: &[BarCmd] = &[
    u("/restore", "aicb.l.restore", "aicb.gemini.restore"),
    u("/copy", "aicb.l.copy", "aicb.gemini.copy"),
];

static PROJECT: &[BarCmd] = &[u("/init", "aicb.l.init", "aicb.gemini.init")];

static EXT: &[BarCmd] = &[
    u("/mcp", "aicb.l.mcp", "aicb.gemini.mcp"),
    u("/extensions", "aicb.l.extensions", "aicb.gemini.extensions"),
];

static PREF: &[BarCmd] = &[u("/settings", "aicb.l.settings", "aicb.gemini.settings")];

#[cfg(test)]
mod tests {
    /// 표에 적은 i18n 키가 **정말 카탈로그에 있는가.**
    ///
    /// 없으면 화면에 `?` 한 글자가 뜬다. `xtask i18n-keys` 가 전체를 지키지만, 이 표는
    /// 한 번에 여러 개를 더하는 자리라 여기서도 한 번 본다.
    #[test]
    fn 표에_적은_키가_모두_있다() {
        let all: Vec<&crate::aicmdcmds::BarCmd> =
            super::primary().iter().chain(super::groups().iter().flat_map(|g| g.cmds)).collect();
        assert!(all.len() >= 12, "표가 너무 작다({}개)", all.len());
        for cmd in all {
            for key in [cmd.label, cmd.desc] {
                assert_ne!(
                    nabi_i18n::tr(nabi_i18n::Lang::Ko, key),
                    "?",
                    "카탈로그에 없는 키: {key}"
                );
            }
        }
    }
}
