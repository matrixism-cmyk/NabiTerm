//! nabi 진입점 — eframe 부트스트랩.
// 릴리스(설치본)는 GUI 서브시스템 — 실행 시 콘솔(파워셸 로그 창)이 뜨지 않는다.
// 디버그는 콘솔 유지(NABI_LOG 실시간 관찰용).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// egui는 프레임마다 작은 할당을 대량으로 한다(레이아웃 잡·갤리·셰이프). Windows 기본 힙은
// 이 패턴에서 병목이 되어 프레임 시간을 크게 잡아먹는다 — 전역 할당자만 바꿔도 체감이 달라진다.
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod app; mod appnew;

// --- nabiPad 코어는 nabi-editor 크레이트로 이관(T5-1) — 기존 crate:: 경로 유지용 심 ---
mod editbig { pub use nabi_editor::editbig::*; }
mod editbuf { pub use nabi_editor::editbuf::*; }
mod edithex { pub use nabi_editor::edithex::*; }
mod editload { pub use nabi_editor::editload::*; }
mod editor { pub use nabi_editor::editor::*; }
mod editorconvert { pub use nabi_editor::editorconvert::*; }
mod editorextract { pub use nabi_editor::editorextract::*; }
mod editorsyntax { pub use nabi_editor::editorsyntax::*; }
mod editortab { pub use nabi_editor::editortab::*; }
mod encodings { pub use nabi_editor::encodings::*; }
mod humanfmt { pub use nabi_editor::humanfmt::*; }
mod appicon; mod gpu; mod softgl; mod arrange; mod bell; mod browser; mod browserfilter; mod browserfs; mod browsergrid; mod browserinput; mod browserapply;
mod browserclip; mod browsercols; mod browserops; mod browsermenu; mod browserpanel; mod browserplaces; mod browserrows; mod browsercell; mod controlui; mod controlapp; mod filetype;

mod clicks; mod closeconfirm; mod connect; mod connhist; mod connhistui; mod connectsave; mod editsftp; mod forwardui; mod dnd; mod events; mod find; mod fonts; mod fontinstall; mod netinfo; mod drives; mod editorsave;
mod editoropen; mod editorclose; mod editorlsp; mod editorlsp2; mod editorlspreq;

mod fileprops; mod filepropsui; mod filezilla; mod xshell; mod mobaxterm; mod putty;

   
 

 

 

    

   
       

mod floatpanels; mod floatterm; mod linkmenu; mod settingsfont; mod worklayout; mod updatemodal; mod shellintegprompt; mod agentguide; mod aicli; mod aiclipage;
mod envcat; mod envpath; mod shelldetect; mod settingsearch; mod settingsearchui; mod paletteorder; mod cmdhistfilter;
mod cmdhistui; mod sftppreview; mod sftppreviewui; mod autoreply; mod autoreplyrun; mod diffopen; mod diffopenui;
mod autofwd; mod autofwdui; mod backoff; mod supportbundle; mod supportbundleui; mod freespace; mod reachall; mod danger;
mod editorconflict; mod guard; mod guardui; mod inputline; mod lastfail; mod logprune; mod agentkeys; mod reopenclosed;
#[cfg(test)] mod autoreplytest;
#[cfg(test)] mod settingscan;
#[cfg(test)] mod i18nlint; mod envwsl; mod envstate; mod envrun; mod envmgr; mod envmgrui;
mod aiclirun; mod aicliupd; mod aicliver; mod wsairesume; mod aiprof; mod aiprofileui; mod aicmdbar; mod aicmdcmds; mod aicmdclaude; mod aicmdother; mod aicmdmore; mod aimode;
mod trzszui; mod xferbar; mod gpupick; mod tiptrans; mod tipai; mod tipoverlay; mod aistatus; mod agentwatch; mod aidash;

mod sshkey; mod sshauth; mod snippetsend; mod dirjump; mod quickselect; mod cmdhist; mod modal; mod onboarding; mod editorapp; mod encsuggest; mod sftpgrid;
mod replay; mod replayapp; mod sessioncast; mod sessioncastread;
mod sessionlog; mod sessionnote; mod sessenvui; mod sessionctx;
mod extwatch; mod triggers; mod findall; mod findallui; mod findfiles; mod difflines; mod dupfiles; mod largefiles;

mod dirtools; mod replaceui; mod sftpbookmark; mod sshcmd; mod sshjump; mod helppages; mod help; mod hostkeyui; mod knownhostsui; mod importscan; mod importui; mod logview; mod logviewui; mod menu;
mod menuact; mod menuactio; mod palconv; mod palette; mod palettecmds; mod palettedispatch; mod palettekeys; mod pathline; mod quake; mod qcparse; mod paneio; mod panewheel; mod paneurl;
mod osc52policy; mod openhere; mod padrecover; mod padrecoverui; mod paste; mod promptfocus; mod qcbar; mod remotecmd; mod remotecmdui; mod reconnect; mod reconnectsess;
mod renameui; mod scrollbar; mod selection; mod sessionsmenu; mod sessiondel;
mod snippetvars; mod recent; mod sftpact; mod sftpentries; mod sftpentryfmt; mod sftpnav; mod sftppanel; mod sftprestore; mod sftptab; mod shellinteg; mod sidebar; mod sidebarsel; mod tabbar;
mod tiling; mod toolsmenu; mod update; mod updateui; mod viewmenu; mod sftptable; mod sftplist; mod viewmode; mod sftpview; mod syncbrowse;
mod tabsterm; mod termlink; mod telegrambridge; mod telegramheartbeat; mod settingstelegram; mod sftppath; mod sftpxfer; mod sftpfind; mod sftpfindui; mod sftphistory; mod controlsftp; mod splash;
mod sshkeygenui; mod syncplan; mod sftpsyncui; mod sftpwatch; mod aihandoff; mod statuschips; mod sftpqueue; mod sftpqpersist; mod sftpqact; mod sftpdownload; mod sftpperms; mod sftptoolbar;
mod sftpops; mod eventsftp;

mod sftpdiff; mod recentpaths; mod sshconfig; mod sshinclude; mod settings; mod settingslists; mod settingslsp; mod settingsprev; mod settingsui; mod settingsa11y; mod settingsui2; mod shortcuts; mod blocklistui; mod
editspotsui; mod wordcompui; mod autolog; mod copyidui; mod cues; mod egress; mod redact; mod secretscan; mod secretui; mod diffrestore; mod opendoc; mod zipops; mod zipui; mod errkey; mod
panegroup; mod scrollmark; mod scrollmarkui; mod slowcmd;
mod splitmenu; mod statusbar; mod statusfit; mod statusfmt; mod tabmenu; mod tabops; mod tabspawn;
mod theme_ui; mod themeimport; mod themeimport2; mod toast; mod titlebar; mod tabs; mod vault; mod view; mod viewacts; mod winclip; mod windnd; mod whatsnew; mod whatsnewui; mod winpos; mod winscp;
mod windndvirt; mod windndfolder; mod viewportcmd; mod windows; mod workspace; mod workspace2; mod worksnap; mod xfersummary; mod worksnapui; mod backup; mod boottime;
mod broadcastview; mod worktree; mod worktreeui; mod schedspec; mod scheduler; mod schedui;

use app::NabiApp;

/// GUI 서브시스템(릴리스)에서 `nabi cli`가 부모 콘솔에 stdout/stderr를 잇도록 한다.
/// 디버그(콘솔 서브시스템)는 이미 콘솔이 있으므로 no-op.
#[cfg(not(debug_assertions))]
fn attach_parent_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
        fn AttachConsole(pid: u32) -> i32;
    }
    // 이미 콘솔이 있으면 그대로 사용. 없을 때만 부모 콘솔에 붙이기 시도하고,
    // 실패(부모도 콘솔 없음/리다이렉트)하면 조용히 포기한다 — 표준출력이 파일/파이프로
    // 리다이렉트돼 있으면 그쪽으로 정상 출력되고, 아무 데도 없으면 run_cli_safe가 보호한다.
    // SAFETY: 둘 다 인자가 상수인 kernel32 호출이다. GetConsoleWindow는 핸들만 읽고,
    // AttachConsole은 실패 시 0을 돌려줄 뿐 우리 메모리를 건드리지 않는다.
    unsafe {
        if GetConsoleWindow().is_null() {
            let _ = AttachConsole(u32::MAX); // ATTACH_PARENT_PROCESS(-1)
        }
    }
}
#[cfg(debug_assertions)]
fn attach_parent_console() {}

/// `nabi cli`를 실행하되, 콘솔/파이프가 없거나 끊겨 표준출력 쓰기가 실패해 `println`이
/// 패닉하더라도 프로세스가 깨끗이 종료되게 감싼다(실제 pane 콘솔에선 정상 — 비대화형·
/// 리다이렉트 환경 보호). 출력 쓰기 패닉만 조용히 삼키고, 진짜 버그 패닉은 그대로 표시.
fn run_cli_safe(args: &[String]) -> i32 {
    use std::io::Write;
    std::panic::set_hook(Box::new(|info| {
        let p = info.payload();
        let msg = p
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| p.downcast_ref::<&str>().copied())
            .unwrap_or("");
        if !msg.contains("printing to std") {
            let _ = writeln!(std::io::stderr(), "{info}"); // 진짜 패닉만 보고(쓰기 실패는 무시).
        }
    }));
    std::panic::catch_unwind(|| nabi_control::client::run_cli(args)).unwrap_or(0)
}

fn main() -> eframe::Result<()> {
    // 시작 시간 계측 시작(로그 전용) — 느려졌을 때 언제부터인지 알 수 있게.
    let mut boot = crate::boottime::Boot::start();
    // `nabi cli <verb>`: 제어 클라이언트 모드 — GUI 없이 파이프 왕복 후 종료.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("cli") {
        attach_parent_console();
        std::process::exit(run_cli_safe(&args[2..]));
    }
    // 업데이트 도우미: 앱이 끝나기를 기다렸다가 인스톨러를 실행한다(GUI 없음).
    // 셸을 거치지 않으려고 우리 자신을 쓴다 — cmd를 거치면 명령줄 따옴표 규칙에 걸린다
    // (사용자 보고 2026-08-23: "''을(를) 찾을 수 없습니다", "Network path was not found").
    if args.get(1).map(String::as_str) == Some(nabi_release::RUN_AFTER_EXIT) {
        let (pid, exe) = (args.get(2).cloned().unwrap_or_default(), args.get(3).cloned().unwrap_or_default());
        std::process::exit(nabi_release::run_after_exit(&pid, &exe));
    }
    // 탐색기 우클릭 "nabiTerm에서 열기". 이미 떠 있으면 그쪽에 넘기고 조용히 끝낸다 —
    // 창이 두 개 뜨면 사용자가 원한 것이 아니다.
    let mut start_cwd: Option<String> = None;
    match openhere::handle(&args) {
        Some(openhere::Outcome::Delegated) => std::process::exit(0),
        Some(openhere::Outcome::StartHere(p)) => start_cwd = Some(p),
        None => {}
    }
    // `nabi mcp`: stdio MCP 서버(제어 파이프 프록시) — Claude Code 등록:
    // `claude mcp add nabiterm -- nabi.exe mcp` (pane 안에서 상속된 env 사용).
    if args.get(1).map(String::as_str) == Some("mcp") {
        std::process::exit(nabi_control::mcp::run());
    }
    // 앱 뮤텍스: 인스톨러(AppMutex=nabiTermRunning)가 설치 '시작 전에' 실행 중 앱을
    // 감지해 종료를 요청하게 한다 — 파일 교체 중 잠금 오류(code 5) 예방. GUI 프로세스만
    // 잡는다(cli/mcp 단명 모드는 위에서 이미 종료). 핸들은 종료까지 유지(의도적 누수).
    // (로컬 mod windows가 crate를 가리므로 ::windows 절대 경로 사용. HANDLE은 Copy라
    // Drop이 없어 CloseHandle 없이 프로세스 종료까지 열려 있다 — 그게 정확히 원하는 것.)
    // SAFETY: CreateMutexW에 이름만 넘긴다(보안 속성 None). 실패해도 무시하며, 반환 핸들은
    // 프로세스 종료까지 유지할 목적으로 일부러 버린다(위 주석 참고).
    unsafe {
        use ::windows::Win32::System::Threading::CreateMutexW;
        let _ = CreateMutexW(None, false, ::windows::core::w!("nabiTermRunning"));
    }
    // '여기서 열기'로 새로 뜨는 경우 첫 셸을 그 폴더에서 연다.
    if let Some(cwd) = start_cwd.filter(|c| !c.is_empty()) {
        std::env::set_var("NABI_START_CWD", cwd);
    }
    // 제어 평면 디스커버리: 자식 셸들이 상속할 파이프/토큰(서버는 NabiApp::new에서).
    // 이미 설정돼 있으면 존중(외부 테스트 하니스용 — 같은 사용자 권한이라 보안 동등).
    // 나비텀 **안의** 셸에서 나비텀을 또 실행하면 부모의 파이프·토큰이 딸려 온다.
    // 그대로 두면 새 앱이 부모의 주소를 자기 것인 양 기록해 버린다 — 남의 것이면 새로 만든다.
    let inherited = std::env::var("NABI_CONTROL_PIPE").unwrap_or_default();
    if inherited.is_empty() || nabi_control::is_foreign_pipe(&inherited) {
        std::env::set_var("NABI_CONTROL_PIPE", nabi_control::pipe_name());
        std::env::set_var("NABI_CONTROL_TOKEN", nabi_control::gen_token()); // 짝을 함께 바꾼다.
    } else if std::env::var_os("NABI_CONTROL_TOKEN").is_none() {
        std::env::set_var("NABI_CONTROL_TOKEN", nabi_control::gen_token());
    }
    // 콘솔 + 회전 파일 로그(설정 폴더 logs/). 가드는 종료까지 보관해야 플러시됨.
    let log_dir = nabi_config::StorageLayout::resolve()
        .config_file
        .parent()
        .map(|p| p.join("logs"));
    let _log_guard = nabi_log::init(log_dir.as_deref());
    // 동기 crash.log + Windows 네이티브 예외 필터: 버퍼 유실/네이티브 트랩(SIGILL 등)도 기록.
    nabi_log::install_crash_handler(log_dir.as_deref());
    windnd::init_ole(); // 파일 드래그-아웃(DoDragDrop)에 필요한 OLE 초기화.
    // 마지막 창 크기 복원(저장값 없으면 넉넉한 기본).
    let cfg = nabi_config::load(&nabi_config::StorageLayout::resolve());
    let (w, h) = if cfg.appearance.window_w >= 400.0 && cfg.appearance.window_h >= 300.0 {
        (cfg.appearance.window_w, cfg.appearance.window_h)
    } else {
        (1200.0, 760.0)
    };
    // 저장된 자리가 지금 화면 안에 있으면 그대로 복원한다. 모니터를 뽑았거나 배치가
    // 바뀌었으면 보이지 않는 자리가 되므로 기본 자리에 띄운다(winpos가 판정).
    let pos = winpos::restore_pos(&cfg, w, h);
    tracing::info!(w, h, restored = pos.is_some(), "초기 창 크기·위치 적용");
    let mut vp = egui::ViewportBuilder::default()
        .with_title("nabiTerm")
        .with_icon(appicon::butterfly())
        .with_inner_size([w, h])
        .with_min_inner_size([400.0, 300.0]);
    if let Some((x, y)) = pos {
        vp = vp.with_position([x, y]);
    }
    let options = eframe::NativeOptions {
        viewport: vp,
        // eframe 자체 창 상태 복원 비활성 — 최소화 중 스냅샷이 다음 실행을 최소화로
        // 시작시키는 문제 방지. 크기는 config(window_w/h)로 우리가 복원한다.
        persist_window: false,
        // glow(OpenGL) 단일 백엔드는 일부 드라이버에서 화면이 안 보이는 사례가 있어 wgpu로.
        // GPU 없는 VM/헤드리스면 softgl이 소프트웨어 GL(Mesa)을 확인 후 받아 GL 백엔드로.
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: gpu::wgpu_options({
            // 그래픽 초기화 도중 죽으면 다음 실행이 알아채도록 표식을 남긴다(첫 프레임에서 지운다).
            let b = softgl::resolve_backends();
            gpupick::mark_starting();
            b
        }),
        // 디더링은 그라데이션 밴딩용인데 터미널은 평면 채움+글리프뿐이라 이득이 없다.
        // 켜두면 프래그먼트마다 추가 연산 — 약한 GPU·RDP·가상 GPU에서 프레임이 눈에 띄게 준다.
        dithering: false,
        ..Default::default()
    };
    boot.window_ready(); // 설정·로그·그래픽 선택까지 끝났다 — 이제 창을 만든다.
    eframe::run_native(
        "nabi",
        options,
        Box::new(move |cc| {
            let mut app = NabiApp::new(cc);
            app.boot = Some(boot); // 첫 프레임에서 총 시간을 기록한다(update.rs).
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
