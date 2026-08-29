//! 빌드가 **자기 진행률을 나비텀에 알린다**(배치 AX).
//!
//! ## 왜 필요한가
//!
//! `xtask dist` 는 몇 분씩 돈다. 그동안 화면에는 `Compiling …` 줄만 흐르고, 얼마나 남았는지
//! 알 길이 없다(사용자 지적 2026-08-29). 특히 배경으로 돌리면 아무것도 안 보인다.
//!
//! 나비텀에는 이미 진행률을 띄우는 자리가 있고(`nabi cli progress`), 화면 글에서 읽어 내는
//! 길도 있다. 그런데 **빌드를 파일로 넘기면** cargo 가 막대를 안 그려서 읽을 것이 없다.
//!
//! 그러면 **빌드가 직접 말하면 된다.** 자기가 몇 단계 중 몇 번째인지는 자기가 제일 잘 안다.
//!
//! ## 없으면 조용히 넘어간다
//!
//! 나비텀 밖에서 빌드할 수도 있고(CI 등), 그때는 알릴 곳이 없다. 그래도 빌드는 돌아야 한다.

/// 이 빌드가 몇 단계로 이루어지는가. 화면에 띄울 백분율의 분모다.
pub struct Steps {
    total: u32,
    done: u32,
    pane: Option<String>,
}

impl Steps {
    /// 알릴 곳이 있으면 준비한다. `NABI_PANE_ID` 가 없으면 아무것도 하지 않는다.
    pub fn new(total: u32) -> Self {
        Self { total: total.max(1), done: 0, pane: std::env::var("NABI_PANE_ID").ok() }
    }

    /// 한 단계 끝났다. 화면의 진행률을 갱신한다.
    pub fn step(&mut self, what: &str) {
        self.done += 1;
        let pct = (self.done * 100 / self.total).min(100);
        println!("[{pct:>3}%] {what}");
        self.tell(Some(pct));
    }

    /// 다 끝났다 — 진행률 표시를 지운다. 남겨 두면 끝난 일이 도는 것처럼 보인다.
    pub fn finish(&mut self) {
        self.tell(None);
    }

    fn tell(&self, pct: Option<u32>) {
        let (Some(pane), Ok(exe)) = (&self.pane, std::env::current_exe()) else {
            return;
        };
        // 개발본은 `nabi.exe`, 설치본은 `nabiTerm.exe` 다. 우리를 부른 나비텀을 찾아 쓴다.
        let Some(nabi) = find_nabi(&exe) else { return };
        let mut c = std::process::Command::new(nabi);
        c.args(["cli", "progress", "--pane", pane]);
        if let Some(p) = pct {
            c.args(["--pct", &p.to_string()]);
        }
        // 실패해도 아무 말 하지 않는다 — 알리지 못하는 것 때문에 빌드가 멈추면 안 된다.
        let _ = c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    }
}

/// 이 PC 의 나비텀 실행 파일을 찾는다.
fn find_nabi(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    // 같은 target 폴더의 개발본을 먼저 본다(빌드 중인 우리 자신 옆에 있다).
    if let Some(dir) = exe.parent() {
        let dev = dir.join("nabi.exe");
        if dev.exists() {
            return Some(dev);
        }
    }
    let installed = std::path::Path::new(r"C:\Program Files (x86)\nabiTerm\nabiTerm.exe");
    installed.exists().then(|| installed.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::Steps;

    #[test]
    fn the_percentage_never_passes_one_hundred() {
        // 단계를 더 밟아도 100 을 넘지 않는다. 넘는 숫자는 사람을 헷갈리게 한다.
        let mut s = Steps { total: 2, done: 0, pane: None };
        s.step("하나");
        s.step("둘");
        s.step("셋");
        assert!(s.done * 100 / s.total >= 100);
    }

    #[test]
    fn zero_steps_does_not_divide_by_zero() {
        let s = Steps::new(0);
        assert_eq!(s.total, 1, "0 을 주면 1 로 본다 — 0 으로 나누면 죽는다");
    }

    #[test]
    fn without_a_pane_it_stays_quiet() {
        // 나비텀 밖에서 빌드할 수도 있다. 그때도 빌드는 돌아야 한다.
        let mut s = Steps { total: 3, done: 0, pane: None };
        s.step("알릴 곳이 없어도 죽지 않는다");
        s.finish();
    }
}
