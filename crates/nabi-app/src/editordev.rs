//! nabiPad 개발 도구(1) — JWT 디코드·Unix 타임스탬프·URL 파싱·쿼리스트링↔JSON + "개발 도구" 메뉴.
//! jwt.io/CyberChef/DevTools 벤치마킹. 순수 함수(서명 검증은 안 함 — 디코드만).

use nabi_i18n::{tr, Lang};

/// JWT(`header.payload.sig`)의 헤더·페이로드를 base64url 디코드해 보기 좋게 출력.
pub(crate) fn jwt_decode(t: &str) -> String {
    let parts: Vec<&str> = t.trim().split('.').collect();
    if parts.len() < 2 {
        return t.to_string();
    }
    let dec = |p: &str| {
        let json = crate::editorcodec2::base64url_decode(p);
        serde_json::from_str::<serde_json::Value>(&json).ok().and_then(|v| serde_json::to_string_pretty(&v).ok())
    };
    match (dec(parts[0]), dec(parts[1])) {
        (Some(h), Some(p)) => format!("// header\n{h}\n\n// payload\n{p}"),
        _ => t.to_string(),
    }
}

/// Unix epoch 초 → `YYYY-MM-DD HH:MM:SS UTC`.
pub(crate) fn from_unix_time(t: &str) -> String {
    match t.trim().parse::<i64>() {
        Ok(s) => chrono::DateTime::from_timestamp(s, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| t.to_string()),
        Err(_) => t.to_string(),
    }
}

/// Unix epoch 밀리초 → `YYYY-MM-DD HH:MM:SS.mmm UTC`(JS Date.now()/Java 타임스탬프).
pub(crate) fn from_unix_ms(t: &str) -> String {
    match t.trim().parse::<i64>() {
        Ok(ms) => chrono::DateTime::from_timestamp_millis(ms)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string())
            .unwrap_or_else(|| t.to_string()),
        Err(_) => t.to_string(),
    }
}

/// 날짜/시각 문자열(UTC로 간주) → Unix epoch 초. 여러 형식 허용.
pub(crate) fn to_unix_time(t: &str) -> String {
    let s = t.trim();
    for f in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, f) {
            return dt.and_utc().timestamp().to_string();
        }
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp().to_string();
    }
    t.to_string()
}

/// URL 쿼리스트링(`b=2&a=1`)을 키 사전순으로 안정 정렬한다(정규화·비교용).
pub(crate) fn sort_query(t: &str) -> String {
    let mut parts: Vec<&str> = t.trim().split('&').collect();
    parts.sort_by_key(|p| p.split_once('=').map(|(k, _)| k).unwrap_or(p).to_string());
    parts.join("&")
}

/// URL을 구성요소(scheme/user/host/port/path/query/fragment)로 분해해 줄별 출력.
pub(crate) fn url_parse(t: &str) -> String {
    let s = t.trim();
    let (scheme, rest) = s.split_once("://").unwrap_or(("", s));
    let (authority, pathq) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let (user, hostport) = authority.split_once('@').unwrap_or(("", authority));
    let (host, port) = hostport
        .rsplit_once(':')
        .filter(|(_, p)| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or((hostport, ""));
    let (before_frag, frag) = pathq.split_once('#').unwrap_or((pathq, ""));
    let (path, query) = before_frag.split_once('?').unwrap_or((before_frag, ""));
    let mut out = String::new();
    for (k, v) in [("scheme", scheme), ("user", user), ("host", host), ("port", port), ("path", path), ("query", query), ("fragment", frag)] {
        if !v.is_empty() {
            out.push_str(&format!("{k}: {v}\n"));
        }
    }
    if out.is_empty() {
        t.to_string()
    } else {
        out.trim_end().to_string()
    }
}

/// 쿼리스트링(`a=1&b=2`) → JSON 객체(값은 URL 디코드).
pub(crate) fn query_to_json(t: &str) -> String {
    let q = t.trim().trim_start_matches('?');
    let mut map = serde_json::Map::new();
    for pair in q.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let dec = crate::editorconvert::url_decode;
        map.insert(dec(k), serde_json::Value::String(dec(v)));
    }
    if map.is_empty() {
        return t.to_string();
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap_or_else(|_| t.to_string())
}

/// JSON 객체 → 쿼리스트링(키·값 URL 인코드). 객체가 아니면 원문.
pub(crate) fn json_to_query(t: &str) -> String {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(t) else {
        return t.to_string();
    };
    let enc = crate::editorconvert::url_encode;
    map.iter()
        .map(|(k, v)| {
            let val = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{}={}", enc(k), enc(&val))
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// "개발 도구" 서브메뉴 — 시간/URL/색상/문자열/네트워크 하위 그룹으로 분류(2단계 계층).
pub(crate) fn dev_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    use crate::editorcolor as col;
    use crate::editordev2 as d2;
    use crate::editmenugroups::pick;
    let mut picked = None;
    ui.menu_button(tr(lang, "editor.devtime"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.jwt", jwt_decode), ("editor.fromunix", from_unix_time),
            ("editor.tounix", to_unix_time), ("editor.fromunixms", from_unix_ms),
        ]));
    });
    ui.menu_button(tr(lang, "editor.devurl"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.urlparse", url_parse), ("editor.qs2json", query_to_json),
            ("editor.json2qs", json_to_query), ("editor.sortquery", sort_query),
        ]));
    });
    // 색상 변환이 많아 HEX/RGB/HSL 계열과 CMYK/기타로 2분할(과밀 방지).
    ui.menu_button(tr(lang, "editor.devcolorbase"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.hex2rgb", d2::hex_color_to_rgb), ("editor.rgb2hex", d2::rgb_to_hex_color),
            ("editor.hex2hsl", d2::hex_to_hsl), ("editor.hsl2hex", d2::hsl_to_hex),
            ("editor.rgb2hsl", col::rgb_to_hsl), ("editor.hsl2rgb", col::hsl_to_rgb),
            ("editor.hex82rgba", col::hex8_to_rgba), ("editor.rgba2hex8", col::rgba_to_hex8),
            ("editor.normhex", col::normalize_hex),
        ]));
    });
    ui.menu_button(tr(lang, "editor.devcolormore"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.rgb2cmyk", col::rgb_to_cmyk), ("editor.cmyk2rgb", col::cmyk_to_rgb),
            ("editor.invertcolor", col::invert_color), ("editor.grayscale", col::rgb_to_grayscale),
            ("editor.complement", col::complementary),
        ]));
    });
    ui.menu_button(tr(lang, "editor.devstr"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.crc32", d2::crc32_hex), ("editor.sqlesc", d2::sql_escape),
            ("editor.regexesc", d2::regex_escape),
        ]));
    });
    ui.menu_button(tr(lang, "editor.devnet"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.ip2int", d2::ip_to_int), ("editor.int2ip", d2::int_to_ip),
        ]));
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_and_time() {
        // {"alg":"HS256"} . {"sub":"42"} (base64url, 무패딩).
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiI0MiJ9.sig";
        let out = jwt_decode(jwt);
        assert!(out.contains("\"alg\": \"HS256\""), "{out}");
        assert!(out.contains("\"sub\": \"42\""), "{out}");
        assert_eq!(from_unix_time("0"), "1970-01-01 00:00:00 UTC");
        assert_eq!(to_unix_time("1970-01-01 00:00:00"), "0");
        assert_eq!(from_unix_ms("1500"), "1970-01-01 00:00:01.500 UTC");
    }

    #[test]
    fn url_and_query() {
        let p = url_parse("https://u@host.com:8080/a/b?x=1#frag");
        assert!(p.contains("scheme: https") && p.contains("host: host.com") && p.contains("port: 8080"));
        assert!(p.contains("path: /a/b") && p.contains("query: x=1") && p.contains("fragment: frag"));
        assert_eq!(query_to_json("a=1&b=hi"), "{\n  \"a\": \"1\",\n  \"b\": \"hi\"\n}");
        assert_eq!(json_to_query("{\"a\":\"1\"}"), "a=1");
        assert_eq!(sort_query("b=2&a=1&a=0"), "a=1&a=0&b=2"); // 키 정렬(안정).
    }
}
