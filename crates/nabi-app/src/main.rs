//! nabi 진입점 — eframe 부트스트랩.
// 릴리스(설치본)는 GUI 서브시스템 — 실행 시 콘솔(파워셸 로그 창)이 뜨지 않는다.
// 디버그는 콘솔 유지(NABI_LOG 실시간 관찰용).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// egui는 프레임마다 작은 할당을 대량으로 한다(레이아웃 잡·갤리·셰이프). Windows 기본 힙은
// 이 패턴에서 병목이 되어 프레임 시간을 크게 잡아먹는다 — 전역 할당자만 바꿔도 체감이 달라진다.
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod app; mod appnew; mod appicon; mod gpu; mod softgl;
mod arrange; mod bell;
mod browser; mod browserfs; mod browsergrid; mod browserinput; mod browserapply;
mod browserclip; mod browsercols; mod browserops; mod browsermenu; mod browserpanel; mod browserrows; mod browsercell;
mod controlui;
mod filetype;
mod humanfmt;
mod clicks;
mod closeconfirm;
mod connect;
mod connectsave;
mod editsftp;
mod forwardui;
mod dnd;
mod events;
mod find;
mod fonts;
mod fontinstall;
mod netinfo;
mod drives;
mod editor; mod editorsave;
mod editoropen; mod editorclose;
mod editload;
mod editortab;
mod filezilla;
mod mobaxterm;
mod putty;
mod editorstatus;
mod editormenu;
mod editorextra;
mod editorminimap;
mod editoroutline;
mod editorconvert;
mod editmenugroups;
mod editorcolor;
mod editorcodec;
mod editorcodec2;
mod editorcodec4;
mod editoralign;
mod editorfreq;
mod editorwidth; mod editorxml; mod editoruuid; mod editorxform;
mod editorcase; mod editorcomment;
mod editortext;
mod editorlist;
mod editordev;
mod editordev2;
mod editorcodec3;
mod editorcsv; mod editorcsv2;
mod editorhash;
mod editormd5;
mod editornum; mod editornumops;
#[cfg(test)]
mod editornum_tests;
mod edithexops;
mod editorindent;
mod editorsort;
mod editorlines;
mod editorstats;
mod edithex; mod edithexedit; mod edithexfind; mod edithexmenu; mod edithexview;
mod encodings;
mod editorhl; mod editorhlinc; mod editorhlspans; mod editorsyntax;
mod editbig; mod editbuf; mod editbufcol; mod editbufedit; mod editbufkeys; mod editbufmove; mod editbufpaint; mod editbufview;
mod editorfind;
mod editorreplace;
mod floatpanels;
mod floatterm;
mod linkmenu;
mod settingsfont;
mod worklayout;
mod updatemodal;
mod shellintegprompt;
mod agentguide;
mod aistatus;
mod aidash;
mod encsuggest;
mod editbufmenu;
mod editbufxform;
mod sshkey;
mod sshauth;
mod snippetsend;
mod dirjump;
mod quickselect;
mod cmdhist;
mod modal;
mod sessionlog; mod sessionnote; mod sessionctx;
mod extwatch;
mod triggers;
mod findfiles;
mod difflines;
mod dupfiles;
mod largefiles;
mod editorextract;
mod editorlineops; mod editorctx; mod dirtools; mod replaceui; mod sftpbookmark;
mod sshcmd; mod sshjump;
mod helppages;
mod help;
mod hostkeyui;
mod knownhostsui;
mod menu;
mod menuact; mod menuactio;
mod palette; mod palettecmds; mod palettedispatch; mod pathline;
mod quake;
mod qcparse;
mod paneio;
mod paneurl;
mod osc52policy;
mod paste;
mod promptfocus;
mod qcbar;
mod reconnect;
mod renameui;
mod scrollbar;
mod selection;
mod sessionsmenu; mod sessiondel;
mod snippetvars;
mod recent;
mod sftpact;
mod sftpentries; mod sftpentryfmt;
mod sftpnav;
mod sftppanel;
mod sftptab;
mod shellinteg;
mod sidebar;
mod tabbar;
mod tiling;
mod update;
mod updateui;
mod viewmenu;
mod sftptable;
mod sftplist;
mod viewmode;
mod sftpview;
mod syncbrowse;
mod tabsterm; mod termlink; mod telegrambridge; mod settingstelegram; mod editorloc;
mod sftppath;
mod sftpxfer; mod sftpqueue; mod sftpqact;
mod sftpdownload;
mod sftpperms;
mod sftptoolbar;
mod sftpops;
mod eventsftp;
mod editsel;
mod sshconfig;
mod settings;
mod settingslists;
mod settingsprev;
mod settingsui;
mod settingsui2;
mod shortcuts;
mod splitmenu;
mod statusbar; mod statusfmt;
mod tabmenu;
mod tabops; mod tabspawn;
mod theme_ui;
mod themeimport;
mod themeimport2;
mod toast;
mod titlebar;
mod tabs;
mod vault;
mod view; mod viewacts;
mod winclip;
mod windnd;
mod windndvirt;
mod windndfolder;
mod viewportcmd;
mod windows;
mod workspace; mod workspace2;

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
    // `nabi cli <verb>`: 제어 클라이언트 모드 — GUI 없이 파이프 왕복 후 종료.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("cli") {
        attach_parent_console();
        std::process::exit(run_cli_safe(&args[2..]));
    }
    // `nabi mcp`: stdio MCP 서버(제어 파이프 프록시) — Claude Code 등록:
    // `claude mcp add nabiterm -- nabi.exe mcp` (pane 안에서 상속된 env 사용).
    if args.get(1).map(String::as_str) == Some("mcp") {
        std::process::exit(nabi_control::mcp::run());
    }
    // 제어 평면 디스커버리: 자식 셸들이 상속할 파이프/토큰(서버는 NabiApp::new에서).
    // 이미 설정돼 있으면 존중(외부 테스트 하니스용 — 같은 사용자 권한이라 보안 동등).
    if std::env::var_os("NABI_CONTROL_PIPE").is_none() {
        std::env::set_var("NABI_CONTROL_PIPE", nabi_control::pipe_name());
    }
    if std::env::var_os("NABI_CONTROL_TOKEN").is_none() {
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
    tracing::info!(w, h, "초기 창 크기 적용");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("nabiTerm")
            .with_icon(appicon::butterfly())
            .with_inner_size([w, h])
            .with_min_inner_size([400.0, 300.0]),
        // eframe 자체 창 상태 복원 비활성 — 최소화 중 스냅샷이 다음 실행을 최소화로
        // 시작시키는 문제 방지. 크기는 config(window_w/h)로 우리가 복원한다.
        persist_window: false,
        // glow(OpenGL) 단일 백엔드는 일부 드라이버에서 화면이 안 보이는 사례가 있어 wgpu로.
        // GPU 없는 VM/헤드리스면 softgl이 소프트웨어 GL(Mesa)을 확인 후 받아 GL 백엔드로.
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: gpu::wgpu_options(softgl::resolve_backends()),
        // 디더링은 그라데이션 밴딩용인데 터미널은 평면 채움+글리프뿐이라 이득이 없다.
        // 켜두면 프래그먼트마다 추가 연산 — 약한 GPU·RDP·가상 GPU에서 프레임이 눈에 띄게 준다.
        dithering: false,
        ..Default::default()
    };
    eframe::run_native(
        "nabi",
        options,
        Box::new(|cc| Ok(Box::new(NabiApp::new(cc)) as Box<dyn eframe::App>)),
    )
}
