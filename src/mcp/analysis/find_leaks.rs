use crate::stats;

pub fn find_leaks_analysis(
    s: &stats::PlayerStats,
    table_size: &str,
) -> serde_json::Value {
    // Baseline ranges: (stat_name, min, max, hero_val, description_if_low, description_if_high)
    let baselines: Vec<(&str, f64, f64, f64, &str, &str)> = if table_size == "full_ring" {
        vec![
            ("vpip", 15.0, 22.0, s.vpip, "Playing too tight preflop — missing profitable spots", "Playing too many hands preflop — entering pots with weak holdings"),
            ("pfr", 11.0, 18.0, s.pfr, "Not raising enough preflop — too passive, missing value and fold equity", "Raising too wide preflop — overvaluing marginal hands"),
            ("three_bet_pct", 4.0, 9.0, s.three_bet_pct, "3-betting too rarely — letting openers realize equity cheaply", "3-betting too wide — getting called or 4-bet with weak holdings"),
            ("fold_to_three_bet", 40.0, 60.0, s.fold_to_three_bet, "Calling/4-betting too many 3-bets — playing too many pots OOP with capped ranges", "Folding to 3-bets too often — being exploited by light 3-bettors"),
            ("cbet_flop", 50.0, 70.0, s.cbet_flop, "C-betting the flop too rarely — giving up initiative and free cards", "C-betting the flop too often — bluffing into strong ranges"),
            ("cbet_turn", 40.0, 65.0, s.cbet_turn, "Not barreling the turn enough — giving up on semi-bluffs and value", "Double-barreling too often — overcommitting with weak hands"),
            ("fold_to_cbet_flop", 35.0, 55.0, s.fold_to_cbet_flop, "Calling flop c-bets too wide — floating with no equity or plan", "Folding to flop c-bets too much — letting opponents profit with any two cards"),
            ("steal_pct", 25.0, 40.0, s.steal_pct, "Not stealing blinds enough — leaving easy money on the table from late position", "Stealing too wide — getting 3-bet or called OOP with marginal hands"),
            ("went_to_showdown_pct", 22.0, 32.0, s.went_to_showdown_pct, "Going to showdown too rarely — may be over-folding postflop", "Going to showdown too often — calling down too light, paying off value bets"),
            ("won_at_showdown_pct", 48.0, 58.0, s.won_at_showdown_pct, "Winning at showdown too rarely — calling with losing hands or poor hand reading", "Winning at showdown too often — may be folding too many marginal winners before showdown"),
            ("aggression_factor", 1.5, 3.5, s.aggression_factor, "Too passive postflop — calling instead of betting/raising for value or as bluffs", "Too aggressive postflop — over-bluffing or raising without enough value hands"),
            ("cold_call_pct", 5.0, 12.0, s.cold_call_pct, "Cold calling too rarely — 3-betting or folding too much in spots where calling is best", "Cold calling too often — entering pots without initiative, hard to play postflop"),
            ("check_raise_pct", 5.0, 12.0, s.check_raise_pct, "Check-raising too rarely — missing value and bluffing opportunities from OOP", "Check-raising too often — overusing the line, becoming predictable"),
            ("wwsf", 42.0, 52.0, s.wwsf, "Low WWSF — not fighting for pots enough when seeing the flop", "High WWSF — may be winning small pots but losing big ones"),
        ]
    } else {
        vec![
            ("vpip", 22.0, 28.0, s.vpip, "Playing too tight preflop — missing profitable spots in a 6-max game", "Playing too many hands preflop — entering pots with weak holdings"),
            ("pfr", 18.0, 24.0, s.pfr, "Not raising enough preflop — too passive, missing value and fold equity", "Raising too wide preflop — overvaluing marginal hands"),
            ("three_bet_pct", 6.0, 11.0, s.three_bet_pct, "3-betting too rarely — letting openers realize equity cheaply", "3-betting too wide — getting called or 4-bet with weak holdings"),
            ("fold_to_three_bet", 40.0, 60.0, s.fold_to_three_bet, "Calling/4-betting too many 3-bets — playing too many pots OOP with capped ranges", "Folding to 3-bets too often — being exploited by light 3-bettors"),
            ("cbet_flop", 55.0, 75.0, s.cbet_flop, "C-betting the flop too rarely — giving up initiative and free cards", "C-betting the flop too often — bluffing into strong ranges"),
            ("cbet_turn", 45.0, 65.0, s.cbet_turn, "Not barreling the turn enough — giving up on semi-bluffs and value", "Double-barreling too often — overcommitting with weak hands"),
            ("fold_to_cbet_flop", 35.0, 50.0, s.fold_to_cbet_flop, "Calling flop c-bets too wide — floating with no equity or plan", "Folding to flop c-bets too much — letting opponents profit with any two cards"),
            ("steal_pct", 30.0, 45.0, s.steal_pct, "Not stealing blinds enough — leaving easy money on the table from late position", "Stealing too wide — getting 3-bet or called OOP with marginal hands"),
            ("went_to_showdown_pct", 24.0, 34.0, s.went_to_showdown_pct, "Going to showdown too rarely — may be over-folding postflop", "Going to showdown too often — calling down too light, paying off value bets"),
            ("won_at_showdown_pct", 48.0, 58.0, s.won_at_showdown_pct, "Winning at showdown too rarely — calling with losing hands or poor hand reading", "Winning at showdown too often — may be folding too many marginal winners before showdown"),
            ("aggression_factor", 2.0, 4.0, s.aggression_factor, "Too passive postflop — calling instead of betting/raising for value or as bluffs", "Too aggressive postflop — over-bluffing or raising without enough value hands"),
            ("cold_call_pct", 6.0, 14.0, s.cold_call_pct, "Cold calling too rarely — 3-betting or folding too much in spots where calling is best", "Cold calling too often — entering pots without initiative, hard to play postflop"),
            ("check_raise_pct", 6.0, 14.0, s.check_raise_pct, "Check-raising too rarely — missing value and bluffing opportunities from OOP", "Check-raising too often — overusing the line, becoming predictable"),
            ("wwsf", 44.0, 54.0, s.wwsf, "Low WWSF — not fighting for pots enough when seeing the flop", "High WWSF — may be winning small pots but losing big ones"),
        ]
    };

    let vpip_pfr_gap = s.vpip - s.pfr;
    let mut leaks: Vec<serde_json::Value> = Vec::new();

    for (stat_name, min, max, value, low_desc, high_desc) in &baselines {
        if *value < *min {
            let deviation = min - value;
            let severity = if deviation > (max - min) { "major" } else if deviation > (max - min) * 0.5 { "moderate" } else { "minor" };
            leaks.push(serde_json::json!({
                "stat": stat_name,
                "value": format!("{:.1}", value),
                "healthy_range": format!("{:.0}-{:.0}", min, max),
                "direction": "low",
                "severity": severity,
                "explanation": low_desc,
            }));
        } else if *value > *max {
            let deviation = value - max;
            let severity = if deviation > (max - min) { "major" } else if deviation > (max - min) * 0.5 { "moderate" } else { "minor" };
            leaks.push(serde_json::json!({
                "stat": stat_name,
                "value": format!("{:.1}", value),
                "healthy_range": format!("{:.0}-{:.0}", min, max),
                "direction": "high",
                "severity": severity,
                "explanation": high_desc,
            }));
        }
    }

    let gap_max = if table_size == "full_ring" { 7.0 } else { 6.0 };
    if vpip_pfr_gap > gap_max {
        let severity = if vpip_pfr_gap > gap_max * 2.0 { "major" } else if vpip_pfr_gap > gap_max * 1.5 { "moderate" } else { "minor" };
        leaks.push(serde_json::json!({
            "stat": "vpip_pfr_gap",
            "value": format!("{:.1}", vpip_pfr_gap),
            "healthy_range": format!("0-{:.0}", gap_max),
            "direction": "high",
            "severity": severity,
            "explanation": "Large gap between VPIP and PFR — entering too many pots by calling instead of raising. Passive preflop play leads to tough postflop spots without initiative.",
        }));
    }

    if s.limp_pct > 5.0 {
        let severity = if s.limp_pct > 20.0 { "major" } else if s.limp_pct > 10.0 { "moderate" } else { "minor" };
        leaks.push(serde_json::json!({
            "stat": "limp_pct",
            "value": format!("{:.1}", s.limp_pct),
            "healthy_range": "0-5",
            "direction": "high",
            "severity": severity,
            "explanation": "Limping too often — open-raising is almost always superior in No Limit. Limping builds small pots without initiative and invites multiway action.",
        }));
    }

    leaks.sort_by(|a, b| {
        let sev_order = |s: &str| match s { "major" => 0, "moderate" => 1, _ => 2 };
        let sa = sev_order(a["severity"].as_str().unwrap_or("minor"));
        let sb = sev_order(b["severity"].as_str().unwrap_or("minor"));
        sa.cmp(&sb)
    });

    serde_json::json!({
        "table_size": table_size,
        "total_hands": s.hands_played,
        "leaks_found": leaks.len(),
        "leaks": leaks,
        "stats": s,
    })
}
