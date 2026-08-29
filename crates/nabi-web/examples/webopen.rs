//! 내장 브라우저 창을 **직접 띄워 본다**(진단용).
//!
//! ```text
//! cargo run -p nabi-web --example webopen -- https://example.com
//! ```
//!
//! 앱 전체를 띄우지 않고 이 크레이트만 시험한다. 창이 뜨는지, 페이지가 그려지는지,
//! 주소 칸과 단추가 도는지 눈으로 본다.

fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| "example.com".into());
    match nabi_web::runtime::version() {
        Some(v) => println!("WebView2 {v} · {url} 을 연다"),
        None => {
            println!("런타임이 없다 — {}", nabi_web::runtime::INSTALL_HINT);
            return;
        }
    }
    if let Err(e) = nabi_web::open(&url, "nabiTerm 웹") {
        println!("열지 못했다: {e}");
        return;
    }
    // open() 은 바로 돌아온다. 창이 살아 있는 동안 이 프로그램도 살아 있어야 한다.
    let secs: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    println!("{secs}초 동안 열어 둔다.");
    std::thread::sleep(std::time::Duration::from_secs(secs));
}
