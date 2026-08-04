//! 앱 아이콘(나비) — 외부 에셋 없이 절차 생성한 RGBA(창/작업표시줄용).

/// 64×64 나비 아이콘: 시안→보라 그라데이션 날개 4장 + 어두운 몸통.
pub(crate) fn butterfly() -> egui::IconData {
    const S: usize = 64;
    let mut rgba = vec![0u8; S * S * 4];
    // 날개 중심(좌상/좌하/우상/우하)과 반경 — 위 날개가 크고 아래가 작다.
    let wings: [(f32, f32, f32, f32); 4] = [
        (22.0, 24.0, 15.0, 12.5), // 좌상 (cx, cy, rx, ry)
        (42.0, 24.0, 15.0, 12.5), // 우상
        (24.0, 43.0, 11.0, 9.5),  // 좌하
        (40.0, 43.0, 11.0, 9.5),  // 우하
    ];
    for y in 0..S {
        for x in 0..S {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut a = 0.0f32;
            for (cx, cy, rx, ry) in wings {
                let d = ((fx - cx) / rx).powi(2) + ((fy - cy) / ry).powi(2);
                if d < 1.0 {
                    a = a.max(1.0 - d * d); // 가장자리 부드럽게.
                }
            }
            let i = (y * S + x) * 4;
            if a > 0.02 {
                // 좌→우 시안(64,180,230) → 보라(150,110,235) 그라데이션.
                let t = fx / S as f32;
                rgba[i] = (64.0 + (150.0 - 64.0) * t) as u8;
                rgba[i + 1] = (180.0 + (110.0 - 180.0) * t) as u8;
                rgba[i + 2] = (230.0 + (235.0 - 230.0) * t) as u8;
                rgba[i + 3] = (a.min(1.0) * 255.0) as u8;
            }
            // 몸통: 중앙 세로 캡슐(날개 위에 덧그림).
            let bd = ((fx - 32.0) / 2.6).powi(2) + ((fy - 33.0) / 16.0).powi(2);
            if bd < 1.0 {
                rgba[i] = 30;
                rgba[i + 1] = 36;
                rgba[i + 2] = 48;
                rgba[i + 3] = 255;
            }
        }
    }
    egui::IconData {
        rgba,
        width: S as u32,
        height: S as u32,
    }
}
