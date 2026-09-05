//! **오류 문구**(T8-1) — 코드마다 세 나라 말.
//!
//! 키는 `err.<코드>` 다. 코드는 오류가 생긴 자리가 정하고(`nabi_error::Coded`), 여기서
//! 말이 된다. 자리표시자는 **번호**(`{0}` `{1}`)로 적는다 — 이름표는 옮겨 적다 틀리면
//! 그 말에서만 값이 조용히 빠진다.
//!
//! 어순은 말마다 다르니 번호 순서를 바꿔도 된다.

/// (키, 영어, 한국어, 일본어)
pub(crate) const ENTRIES: &[(&str, &str, &str, &str)] = &[
    // ── 셸 ──────────────────────────────────────────────────────────
    (
        "err.shell.notfound",
        "Cannot find the shell program: {0}. It does not appear to be installed.",
        "셸 프로그램을 찾지 못했습니다: {0}. 설치되어 있지 않은 것 같습니다.",
        "シェルプログラムが見つかりません: {0}。インストールされていないようです。",
    ),
    (
        "err.shell.storealias",
        "{0} is the Microsoft Store version and this account has no license for it, so it cannot run. Tools \u{25b8} Environment manager can install PowerShell 7 as a normal program. Original error: {1}",
        "{0}은(는) Microsoft Store판이라 이 계정에 앱 라이선스가 없어 실행되지 않습니다. 도구 \u{25b8} 환경 관리자에서 PowerShell 7을 설치하면 정식 설치본으로 열립니다. 원문: {1}",
        "{0} は Microsoft Store 版で、このアカウントにライセンスがないため起動できません。ツール \u{25b8} 環境マネージャーから PowerShell 7 を導入できます。原文: {1}",
    ),
    (
        "err.shell.spawn",
        "Could not start the shell {0}: {1}",
        "셸 {0}을(를) 시작하지 못했습니다: {1}",
        "シェル {0} を起動できませんでした: {1}",
    ),
    // ── 제어 평면 ───────────────────────────────────────────────────
    (
        "err.control.pipe",
        "Could not reach nabiTerm on {0}: {1}. Is nabiTerm running, and was this shell started by it?",
        "나비텀에 닿지 못했습니다({0}): {1}. 나비텀이 실행 중이고, 이 셸을 나비텀이 띄운 것이 맞나요?",
        "nabiTerm に接続できません({0}): {1}。nabiTerm は起動していますか、このシェルは nabiTerm から開いたものですか。",
    ),
    (
        "err.control.timeout",
        "nabiTerm did not answer on {0} in time.",
        "나비텀이 제때 답하지 않았습니다({0}).",
        "nabiTerm が時間内に応答しませんでした({0})。",
    ),
];
