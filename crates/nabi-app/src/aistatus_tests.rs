//! `aistatus.rs` 의 시험 — 줄 한도로 갈라 뒀다(배치 AX).
//!
//! 시험은 코드가 아니라 **약속**이다. 그 약속을 지우면서 한도를 맞추면 안 되므로,
//! 파일을 나눌 때 시험을 먼저 옮긴다.

use super::*;


    #[test]
    fn detects_ai_commands() {
        assert!(is_ai_command("claude"));
        assert!(is_ai_command("C:\\Users\\u\\claude.exe --resume"));
        assert!(is_ai_command("aider"));
        assert!(is_ai_command("opencode")); // 추가 AI CLI.
        assert!(is_ai_command("ollama run llama3"));
        assert!(!is_ai_command("vim notes.txt"));
        assert!(!is_ai_command("git status"));
    }

    #[test]
    fn token_usage() {
        assert_eq!(parse_token_usage("42k/200k"), Some(0.21));
        assert_eq!(parse_token_usage("100000/200000"), Some(0.5));
        assert_eq!(parse_token_usage("1.5m/3m"), Some(0.5));
        assert_eq!(parse_token_usage("42,000/200,000"), Some(0.21)); // 천 단위 쉼표.
        assert_eq!(parse_token_usage("nope"), None);
    }

    #[test]
    fn elapsed_fmt() {
        // 모양은 이제 `statusfmt::human_secs` 하나가 정한다(배치 AD).
        assert_eq!(human_elapsed(45), "45s");
        assert_eq!(human_elapsed(3 * 60 + 12), "3m 12s");
        assert_eq!(human_elapsed(3600 + 2 * 60), "1h 02m");
    }

    #[test]
    fn display_from_run_cmd() {
        let d = ai_display(None, Some("claude --resume"), Some(Duration::from_secs(72)), None).unwrap();
        assert!(d.label.contains("claude") && d.label.contains("1m 12s"));
        assert!(ai_display(None, Some("ls -la"), None, None).is_none());
    }

    #[test]
    fn display_from_pane_status() {
        let mut m = BTreeMap::new();
        m.insert("model".into(), "opus".into());
        m.insert("tokens".into(), "50k/200k".into());
        let d = ai_display(Some(&m), None, None, None).unwrap();
        assert_eq!(d.gauge, Some(0.25));
        assert!(d.label.contains("opus"));
    }

    #[test]
    fn cost_parsing() {
        assert_eq!(parse_cost("$1.40"), Some(1.40));
        assert_eq!(parse_cost("1.4 USD"), Some(1.4));
        assert_eq!(parse_cost("0.12"), Some(0.12));
        assert_eq!(parse_cost("$1,234.50"), Some(1234.5)); // 천 단위 쉼표.
        assert_eq!(parse_cost("free"), None);
    }

    #[test]
    fn context_threshold() {
        assert!(context_alert(Some(0.85), 0.8));
        assert!(context_alert(Some(0.8), 0.8));
        assert!(!context_alert(Some(0.5), 0.8));
        assert!(!context_alert(None, 0.8));
    }

    #[test]
    fn agent_states() {
        let mut m = BTreeMap::new();
        assert_eq!(agent_state(&m, false), 0); // idle
        assert_eq!(agent_state(&m, true), 1); // working
        m.insert("state".to_string(), "waiting for input".to_string());
        assert_eq!(agent_state(&m, true), 2); // blocked 우선
        m.insert("state".to_string(), "thinking".to_string());
        assert_eq!(agent_state(&m, true), 1); // 비차단 상태면 working
    }

    #[test]
    fn aggregate_costs() {
        let mut a = BTreeMap::new();
        a.insert("cost".to_string(), "$1.40".to_string());
        a.insert("tokens".to_string(), "100k/200k".to_string());
        let mut b = BTreeMap::new();
        b.insert("cost".to_string(), "$0.60".to_string());
        b.insert("tokens".to_string(), "180k/200k".to_string());
        let empty = BTreeMap::new();
        let agg = aggregate([&a, &b, &empty].into_iter());
        assert_eq!(agg.panes, 2); // 빈 상태 제외
        assert!((agg.total_cost - 2.0).abs() < 1e-4);
        assert!((agg.max_gauge - 0.9).abs() < 1e-4);
    }

    #[test]
    fn context_tiers() {
        assert_eq!(context_tier(0.5), 0);
        assert_eq!(context_tier(0.79), 0);
        assert_eq!(context_tier(0.8), 1);
        assert_eq!(context_tier(0.94), 1);
        assert_eq!(context_tier(0.95), 2);
        assert_eq!(context_tier(1.0), 2);
    }

    #[test]
    fn parts_meaningful_order() {
        // BTreeMap 알파벳 순(burn,cost,model,tokens)이지만 표시는 model→tokens→cost→burn.
        let mut m = BTreeMap::new();
        m.insert("status".into(), "thinking".into());
        m.insert("tokens".into(), "50k/200k".into());
        m.insert("cost".into(), "$1.40".into());
        m.insert("model".into(), "opus".into());
        let p = ordered_parts(&m);
        assert_eq!(p[0], "opus");
        assert_eq!(p[1], "50k/200k");
        assert!(p[2].contains("1.40"));
        assert_eq!(p[3], "thinking");
    }

    /// 이 줄을 보는 사람이 가장 먼저 궁금한 것은 "지금 뭐 하는 중이고 얼마나 남았나"다.
    #[test]
    fn the_task_and_progress_show_up() {
        let mut m = BTreeMap::new();
        m.insert("model".into(), "opus".into());
        m.insert("task".into(), "v0.1.494 배포".into());
        let d = ai_display(Some(&m), None, None, Some(60)).unwrap();
        assert!(d.label.contains("v0.1.494 배포"), "{}", d.label);
        assert!(d.label.contains("60%"), "{}", d.label);
        // 작업 이름이 모델보다 뒤에 온다 — 순서가 뒤집히면 읽는 차례가 어색하다.
        let (mi, ti) = (d.label.find("opus").unwrap(), d.label.find("v0.1.494").unwrap());
        assert!(mi < ti, "{}", d.label);
    }

    /// 발행 상태가 없어도(셸 통합 자동 감지) 진행률은 보인다.
    #[test]
    fn progress_shows_without_published_status() {
        let d = ai_display(None, Some("claude"), None, Some(35)).unwrap();
        assert!(d.label.contains("35%"), "{}", d.label);
    }

    /// 진행률이 없으면 아무것도 덧붙이지 않는다 — 빈 자리에 구분점만 남으면 지저분하다.
    #[test]
    fn nothing_extra_when_there_is_no_progress() {
        let d = ai_display(None, Some("claude"), None, None).unwrap();
        assert!(!d.label.contains('%'), "{}", d.label);
        assert!(!d.label.trim_end().ends_with('\u{00b7}'), "{}", d.label);
    }
