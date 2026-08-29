//! 화면 캡처 요청을 실제로 처리한다(배치 AN).
//!
//! ## 왜 앱이 하는가
//!
//! 창이 화면 어디에 있는지, 각 탭이 그 안에서 어느 자리를 차지하는지는 **그리는 쪽만**
//! 안다. 제어 평면은 그 값을 모르므로 요청만 넘기고 여기서 처리한다.
//!
//! ## 논리 점과 실제 점
//!
//! egui 가 말하는 자리는 **논리 점**이다. 화면 배율이 125% 면 실제 점은 그보다 크다.
//! 화면에서 읽을 때는 실제 점을 써야 하므로 배율을 곱한다. 이걸 빠뜨리면 배율을 올린
//! PC 에서 엉뚱한 자리가 찍힌다.

use nabi_types::PaneId;

impl crate::app::NabiApp {
    /// 화면을 뜬다. 남긴 경로 또는 실패 이유를 돌려준다.
    pub(crate) fn take_screenshot(
        &mut self,
        ctx: &egui::Context,
        pane: Option<u64>,
        out: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let hwnd = self.hwnd.ok_or("창 손잡이를 아직 모른다")?;
        let (wx, wy, ww, wh) = crate::screenshot::window_rect(hwnd).ok_or("창 자리를 읽지 못했다")?;
        let (x, y, w, h) = match pane {
            None => (wx, wy, ww, wh),
            Some(id) => {
                let r = self
                    .pane_rects
                    .get(&PaneId::new(id))
                    .copied()
                    .ok_or_else(|| format!("pane {id} 이 이번 화면에 그려지지 않았다"))?;
                let s = ctx.pixels_per_point();
                // 논리 점 → 실제 점. 창 왼쪽 위를 기준으로 잡혀 있으므로 창 자리를 더한다.
                (
                    wx + (r.min.x * s).round() as i32,
                    wy + (r.min.y * s).round() as i32,
                    (r.width() * s).round() as i32,
                    (r.height() * s).round() as i32,
                )
            }
        };
        let path = match out {
            Some(p) => std::path::PathBuf::from(p),
            None => default_path(pane),
        };
        crate::screenshot::grab(&path, x, y, w, h)?;
        Ok(path)
    }
}

/// 어디에 남길지 안 알려 줬을 때의 자리.
///
/// 시각을 이름에 넣는다 — 여러 번 찍으면 앞의 것을 덮어써서 비교할 수 없게 된다.
fn default_path(pane: Option<u64>) -> std::path::PathBuf {
    let now = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let who = match pane {
        Some(p) => format!("pane{p}"),
        None => "window".into(),
    };
    std::env::temp_dir().join("nabi-shots").join(format!("{who}-{now}.png"))
}

#[cfg(test)]
mod tests {
    use super::default_path;

    #[test]
    fn each_shot_gets_its_own_name() {
        // 같은 이름이면 앞의 것을 덮어써서 전후를 비교할 수 없다.
        let a = default_path(Some(3));
        assert!(a.to_string_lossy().contains("pane3"), "{a:?}");
        assert!(a.extension().is_some_and(|e| e == "png"));
        assert!(default_path(None).to_string_lossy().contains("window"));
    }
}
