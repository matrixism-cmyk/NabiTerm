//! 스케줄 사양 파서·발화 판정(C3, 순수). `"*/5 * * * *"` · `"every 15m"` · `"at 09:30"`.
//!
//! cron 5필드(분 시 일 월 요일)를 표준 의미로 판정한다: 필드는 `*`·숫자·`a-b`·`*/n`·콤마
//! 목록. 일(DOM)과 요일(DOW)이 **둘 다** 제한되면 OR(표준 cron 관례). 초 단위는 없다 —
//! 발화는 분 granularity(같은 분에 한 번).

/// 파싱된 스케줄.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Spec {
    /// cron 5필드(각 필드는 허용값 집합, None=무제한).
    Cron(Box<CronSpec>),
    /// N분마다(마지막 실행 기준).
    Every(u64),
    /// 매일 HH:MM.
    At { h: u32, m: u32 },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct CronSpec {
    pub minute: Option<Vec<u32>>,
    pub hour: Option<Vec<u32>>,
    pub dom: Option<Vec<u32>>,
    pub month: Option<Vec<u32>>,
    pub dow: Option<Vec<u32>>, // 0=일요일(chrono weekday.num_days_from_sunday와 일치).
}

/// 사양 문자열 파싱. 형식 오류는 Err(사용자에게 그대로 보여줄 문구).
pub(crate) fn parse(spec: &str) -> Result<Spec, String> {
    let s = spec.trim();
    if let Some(rest) = s.strip_prefix("every ").or_else(|| s.strip_prefix("매 ")) {
        let rest = rest.trim();
        let (num, unit) = rest.split_at(rest.len().saturating_sub(1));
        let n: u64 = num.trim().parse().map_err(|_| format!("주기 형식 오류: {rest}"))?;
        let mins = match unit {
            "m" | "분" => n,
            "h" => n * 60,
            _ => return Err(format!("단위는 m/h: {rest}")),
        };
        if mins == 0 {
            return Err("주기는 1분 이상".into());
        }
        return Ok(Spec::Every(mins));
    }
    if let Some(rest) = s.strip_prefix("at ") {
        let (h, m) = rest.trim().split_once(':').ok_or_else(|| format!("HH:MM 형식: {rest}"))?;
        let (h, m): (u32, u32) = (h.parse().map_err(|_| "시 오류")?, m.parse().map_err(|_| "분 오류")?);
        if h > 23 || m > 59 {
            return Err("시각 범위 오류".into());
        }
        return Ok(Spec::At { h, m });
    }
    let fields: Vec<&str> = s.split_whitespace().collect();
    if fields.len() != 5 {
        return Err("cron 5필드(분 시 일 월 요일) 또는 'every 15m' / 'at 09:30'".into());
    }
    let f = |txt: &str, lo: u32, hi: u32| -> Result<Option<Vec<u32>>, String> { parse_field(txt, lo, hi) };
    Ok(Spec::Cron(Box::new(CronSpec {
        minute: f(fields[0], 0, 59)?,
        hour: f(fields[1], 0, 23)?,
        dom: f(fields[2], 1, 31)?,
        month: f(fields[3], 1, 12)?,
        dow: f(fields[4], 0, 7).map(|v| v.map(normalize_dow))?,
    })))
}

/// cron 한 필드: `*`→None, `*/n`·`a-b`·`a,b,c` 조합 → 허용값 목록.
fn parse_field(txt: &str, lo: u32, hi: u32) -> Result<Option<Vec<u32>>, String> {
    if txt == "*" {
        return Ok(None);
    }
    let mut out = Vec::new();
    for part in txt.split(',') {
        if let Some(step) = part.strip_prefix("*/") {
            let n: u32 = step.parse().map_err(|_| format!("스텝 오류: {part}"))?;
            if n == 0 { return Err("스텝 0 불가".into()); }
            out.extend((lo..=hi).filter(|v| (v - lo).is_multiple_of(n)));
        } else if let Some((a, b)) = part.split_once('-') {
            let (a, b): (u32, u32) = (a.parse().map_err(|_| format!("범위 오류: {part}"))?, b.parse().map_err(|_| format!("범위 오류: {part}"))?);
            if a > b || b > hi || a < lo { return Err(format!("범위 밖: {part}")); }
            out.extend(a..=b);
        } else {
            let v: u32 = part.parse().map_err(|_| format!("숫자 오류: {part}"))?;
            if v < lo || v > hi { return Err(format!("범위 밖: {part}")); }
            out.push(v);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(Some(out))
}

/// DOW 7=일요일 표기를 0으로 접는다.
fn normalize_dow(mut v: Vec<u32>) -> Vec<u32> {
    for x in v.iter_mut() {
        if *x == 7 { *x = 0; }
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// 이 분(minute)에 발화하는가. `last_fire_min`=마지막 발화의 분 식별자(중복 발화 방지,
/// Every는 경과 기준). now는 로컬 시각.
pub(crate) fn due(spec: &Spec, now: &chrono::DateTime<chrono::Local>, last_fire_min: Option<i64>) -> bool {
    use chrono::{Datelike, Timelike};
    let cur_min = now.timestamp() / 60;
    if last_fire_min == Some(cur_min) {
        return false; // 같은 분에 이미 발화.
    }
    match spec {
        Spec::Every(mins) => match last_fire_min {
            None => true, // 첫 틱에 한 번 돌고 이후 주기.
            Some(last) => cur_min - last >= *mins as i64,
        },
        Spec::At { h, m } => now.hour() == *h && now.minute() == *m,
        Spec::Cron(c) => {
            let ok = |set: &Option<Vec<u32>>, v: u32| set.as_ref().is_none_or(|s| s.contains(&v));
            let base = ok(&c.minute, now.minute()) && ok(&c.hour, now.hour()) && ok(&c.month, now.month());
            // DOM·DOW 둘 다 제한이면 OR(표준 cron).
            let dom_ok = ok(&c.dom, now.day());
            let dow_ok = ok(&c.dow, now.weekday().num_days_from_sunday());
            let date_ok = match (&c.dom, &c.dow) {
                (Some(_), Some(_)) => dom_ok || dow_ok,
                _ => dom_ok && dow_ok,
            };
            base && date_ok
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> chrono::DateTime<chrono::Local> {
        chrono::Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn parses_three_forms() {
        assert_eq!(parse("every 15m").unwrap(), Spec::Every(15));
        assert_eq!(parse("every 2h").unwrap(), Spec::Every(120));
        assert_eq!(parse("at 09:30").unwrap(), Spec::At { h: 9, m: 30 });
        assert!(matches!(parse("*/5 * * * *").unwrap(), Spec::Cron(_)));
        assert!(parse("every 0m").is_err());
        assert!(parse("at 25:00").is_err());
        assert!(parse("* * * *").is_err(), "4필드는 거부");
        assert!(parse("61 * * * *").is_err(), "범위 밖 거부");
    }

    #[test]
    fn cron_fields_match() {
        let s = parse("*/15 9-17 * * 1-5").unwrap(); // 평일 9~17시 15분마다.
        assert!(due(&s, &t(2026, 8, 17, 9, 0), None)); // 월요일.
        assert!(due(&s, &t(2026, 8, 17, 17, 45), None));
        assert!(!due(&s, &t(2026, 8, 17, 8, 45), None), "시간 밖");
        assert!(!due(&s, &t(2026, 8, 16, 9, 0), None), "일요일 제외");
        assert!(!due(&s, &t(2026, 8, 17, 9, 7), None), "15분 배수 아님");
    }

    /// DOM과 DOW가 둘 다 제한되면 OR — 표준 cron 관례(모르면 둘 다 만족을 기대하게 된다).
    #[test]
    fn dom_dow_both_restricted_is_or() {
        let s = parse("0 0 13 * 5").unwrap(); // 13일 또는 금요일.
        assert!(due(&s, &t(2026, 8, 13, 0, 0), None), "13일(목)이라도 발화");
        assert!(due(&s, &t(2026, 8, 14, 0, 0), None), "금요일이라 발화");
        assert!(!due(&s, &t(2026, 8, 15, 0, 0), None), "둘 다 아님");
    }

    /// 같은 분 중복 발화 금지 + every는 경과 기준.
    #[test]
    fn dedup_and_every_semantics() {
        let s = parse("* * * * *").unwrap();
        let now = t(2026, 8, 16, 10, 0);
        let cur = now.timestamp() / 60;
        assert!(due(&s, &now, None));
        assert!(!due(&s, &now, Some(cur)), "같은 분 재발화 금지");
        let e = parse("every 30m").unwrap();
        assert!(due(&e, &now, Some(cur - 30)));
        assert!(!due(&e, &now, Some(cur - 29)));
    }

    #[test]
    fn dow_seven_is_sunday() {
        let s = parse("0 9 * * 7").unwrap();
        assert!(due(&s, &t(2026, 8, 16, 9, 0), None), "7=일요일");
    }
}
