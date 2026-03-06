use crate::sessions;
use crate::stats;
use crate::types::Hand;

pub fn detect_tilt_analysis(
    hands: Vec<Hand>,
    hero: &str,
    threshold: f64,
    min_hands: usize,
) -> serde_json::Value {
    let baseline = stats::calculate_stats(&hands, hero);
    let all_sessions = sessions::detect_sessions(hands, hero);

    if all_sessions.is_empty() {
        return serde_json::json!({
            "tilt_sessions": [],
            "total_sessions": 0,
            "message": "No cash game sessions detected.",
        });
    }

    let mut tilt_sessions: Vec<serde_json::Value> = Vec::new();

    for session in &all_sessions {
        if session.total_hands < min_hands {
            continue;
        }

        let session_hands: Vec<Hand> = session
            .tables
            .iter()
            .flat_map(|t| t.hands.iter().cloned())
            .collect();

        let session_stats = stats::calculate_stats(&session_hands, hero);

        let checks: Vec<(&str, f64, f64, &str)> = vec![
            ("vpip", session_stats.vpip, baseline.vpip, "VPIP spike suggests playing too many hands — possible frustration-driven looseness"),
            ("pfr", session_stats.pfr, baseline.pfr, "PFR deviation — raising pattern changed significantly from baseline"),
            ("three_bet_pct", session_stats.three_bet_pct, baseline.three_bet_pct, "3-bet frequency changed — may indicate revenge-raising or over-tightening"),
            ("aggression_factor", session_stats.aggression_factor, baseline.aggression_factor, "Aggression factor shift — possible switch to overly aggressive or overly passive play"),
            ("went_to_showdown_pct", session_stats.went_to_showdown_pct, baseline.went_to_showdown_pct, "Showdown frequency changed — calling down too light (tilt) or folding too much (scared)"),
            ("cbet_flop", session_stats.cbet_flop, baseline.cbet_flop, "C-bet frequency shifted — autopilot betting or giving up too easily"),
            ("wwsf", session_stats.wwsf, baseline.wwsf, "WWSF changed — fighting for pots differently than usual"),
            ("fold_to_cbet_flop", session_stats.fold_to_cbet_flop, baseline.fold_to_cbet_flop, "Fold-to-cbet changed — stubbornly calling or over-folding"),
        ];

        let mut deviations: Vec<serde_json::Value> = Vec::new();
        for (stat_name, session_val, baseline_val, explanation) in &checks {
            if !baseline_val.is_finite() || !session_val.is_finite() {
                continue;
            }
            let diff = session_val - baseline_val;
            if diff.abs() >= threshold {
                let direction = if diff > 0.0 { "higher" } else { "lower" };
                deviations.push(serde_json::json!({
                    "stat": stat_name,
                    "session_value": format!("{:.1}", session_val),
                    "baseline_value": format!("{:.1}", baseline_val),
                    "deviation": format!("{:+.1}", diff),
                    "direction": direction,
                    "explanation": explanation,
                }));
            }
        }

        let mut worst_streak = 0i32;
        let mut current_streak = 0i32;
        for hand in &session_hands {
            let profit = stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
            if profit < 0.0 {
                current_streak += 1;
                worst_streak = worst_streak.max(current_streak);
            } else {
                current_streak = 0;
            }
        }

        if !deviations.is_empty() || worst_streak >= 8 {
            let mut entry = serde_json::json!({
                "session_id": session.session_id,
                "start_time": session.start_time,
                "end_time": session.end_time,
                "duration_minutes": session.duration_minutes,
                "hands": session.total_hands,
                "net_profit": format!("{:.2}", session.net_profit),
                "net_profit_bb": format!("{:.1}", session.net_profit_bb),
                "winrate_bb100": format!("{:.1}", session_stats.winrate_bb100),
                "deviations": deviations,
            });

            if worst_streak >= 5 {
                entry.as_object_mut().unwrap().insert(
                    "worst_loss_streak".to_string(),
                    serde_json::json!(worst_streak),
                );
            }

            tilt_sessions.push(entry);
        }
    }

    tilt_sessions.sort_by(|a, b| {
        let da = a["deviations"].as_array().map(|v| v.len()).unwrap_or(0);
        let db = b["deviations"].as_array().map(|v| v.len()).unwrap_or(0);
        db.cmp(&da)
    });

    serde_json::json!({
        "threshold_pct_points": threshold,
        "min_hands_per_session": min_hands,
        "total_sessions_analyzed": all_sessions.iter().filter(|s| s.total_hands >= min_hands).count(),
        "tilt_sessions_found": tilt_sessions.len(),
        "baseline_stats": {
            "total_hands": baseline.hands_played,
            "vpip": format!("{:.1}", baseline.vpip),
            "pfr": format!("{:.1}", baseline.pfr),
            "three_bet_pct": format!("{:.1}", baseline.three_bet_pct),
            "aggression_factor": format!("{:.2}", baseline.aggression_factor),
            "went_to_showdown_pct": format!("{:.1}", baseline.went_to_showdown_pct),
            "cbet_flop": format!("{:.1}", baseline.cbet_flop),
            "wwsf": format!("{:.1}", baseline.wwsf),
            "fold_to_cbet_flop": format!("{:.1}", baseline.fold_to_cbet_flop),
        },
        "tilt_sessions": tilt_sessions,
    })
}
