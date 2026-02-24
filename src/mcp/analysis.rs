use std::collections::HashMap;

use crate::sessions;
use crate::stats;
use crate::types::{ActionType, Card, Hand, Street};

use super::helpers::{combo_label, days_from_ymd, rank_order, ymd_from_days};

// ── find_leaks ──────────────────────────────────────────────────────────────

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

// ── detect_tilt ─────────────────────────────────────────────────────────────

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

// ── get_street_stats ────────────────────────────────────────────────────────

pub fn get_street_stats_analysis(hands: &[Hand], player: &str) -> serde_json::Value {
    struct StreetCounts {
        hands_seen: u64,
        bets: u64,
        raises: u64,
        calls: u64,
        checks: u64,
        folds: u64,
    }

    impl StreetCounts {
        fn new() -> Self {
            Self { hands_seen: 0, bets: 0, raises: 0, calls: 0, checks: 0, folds: 0 }
        }
        fn total_actions(&self) -> u64 {
            self.bets + self.raises + self.calls + self.checks + self.folds
        }
        fn aggressive_actions(&self) -> u64 {
            self.bets + self.raises
        }
    }

    let mut flop = StreetCounts::new();
    let mut turn = StreetCounts::new();
    let mut river = StreetCounts::new();

    let streets_of_interest = [Street::Flop, Street::Turn, Street::River];

    for hand in hands {
        let in_hand = hand.players.iter().any(|p| p.name == player && !p.is_sitting_out);
        if !in_hand { continue; }

        for &street in &streets_of_interest {
            let player_acted = hand.actions.iter().any(|a| a.player == player && a.street == street);
            if player_acted {
                let counts = match street {
                    Street::Flop => &mut flop,
                    Street::Turn => &mut turn,
                    Street::River => &mut river,
                    _ => continue,
                };
                counts.hands_seen += 1;
            }
        }

        for action in &hand.actions {
            if action.player != player { continue; }
            let counts = match action.street {
                Street::Flop => &mut flop,
                Street::Turn => &mut turn,
                Street::River => &mut river,
                _ => continue,
            };
            match &action.action_type {
                ActionType::Bet { .. } => counts.bets += 1,
                ActionType::Raise { .. } => counts.raises += 1,
                ActionType::Call { .. } => counts.calls += 1,
                ActionType::Check => counts.checks += 1,
                ActionType::Fold => counts.folds += 1,
                _ => {}
            }
        }
    }

    let pct = |num: u64, den: u64| -> f64 {
        if den > 0 { num as f64 / den as f64 * 100.0 } else { 0.0 }
    };

    let street_json = |name: &str, c: &StreetCounts| -> serde_json::Value {
        let total = c.total_actions();
        let agg = c.aggressive_actions();
        let af = if c.calls > 0 { agg as f64 / c.calls as f64 } else if agg > 0 { f64::INFINITY } else { 0.0 };
        serde_json::json!({
            "street": name,
            "hands_seen": c.hands_seen,
            "total_actions": total,
            "bets": c.bets,
            "raises": c.raises,
            "calls": c.calls,
            "checks": c.checks,
            "folds": c.folds,
            "bet_pct": format!("{:.1}", pct(c.bets, total)),
            "raise_pct": format!("{:.1}", pct(c.raises, total)),
            "call_pct": format!("{:.1}", pct(c.calls, total)),
            "check_pct": format!("{:.1}", pct(c.checks, total)),
            "fold_pct": format!("{:.1}", pct(c.folds, total)),
            "aggression_pct": format!("{:.1}", pct(agg, total)),
            "aggression_factor": if af.is_infinite() { "inf".to_string() } else { format!("{:.2}", af) },
        })
    };

    let total_hands = hands.iter()
        .filter(|h| h.players.iter().any(|p| p.name == player && !p.is_sitting_out))
        .count();

    serde_json::json!({
        "player": player,
        "total_hands": total_hands,
        "streets": [
            street_json("flop", &flop),
            street_json("turn", &turn),
            street_json("river", &river),
        ],
    })
}

// ── get_sizing_profile ──────────────────────────────────────────────────────

pub fn get_sizing_profile_analysis(hands: &[Hand], player: &str) -> serde_json::Value {
    fn size_bucket(bet_amount: f64, pot_before: f64) -> &'static str {
        if pot_before <= 0.0 { return "unknown"; }
        let ratio = bet_amount / pot_before;
        if ratio < 0.30 { "tiny (<30%)" }
        else if ratio < 0.40 { "third (30-40%)" }
        else if ratio < 0.55 { "half (40-55%)" }
        else if ratio < 0.72 { "two-thirds (55-72%)" }
        else if ratio < 0.88 { "three-quarters (72-88%)" }
        else if ratio < 1.15 { "pot (88-115%)" }
        else { "overbet (>115%)" }
    }

    struct StreetSizing {
        total_bets: u64,
        buckets: HashMap<&'static str, u64>,
        sizes_pct: Vec<f64>,
    }

    impl StreetSizing {
        fn new() -> Self {
            Self { total_bets: 0, buckets: HashMap::new(), sizes_pct: Vec::new() }
        }
    }

    let mut preflop = StreetSizing::new();
    let mut flop = StreetSizing::new();
    let mut turn = StreetSizing::new();
    let mut river = StreetSizing::new();
    let mut total_sizing_actions = 0u64;

    for hand in hands {
        let in_hand = hand.players.iter().any(|p| p.name == player && !p.is_sitting_out);
        if !in_hand { continue; }

        let mut pot = 0.0f64;
        let mut round_invested: HashMap<&str, f64> = HashMap::new();
        let mut current_street = Street::Preflop;

        for action in &hand.actions {
            if action.street != current_street {
                current_street = action.street;
                round_invested.clear();
            }

            let pot_before = pot;

            match &action.action_type {
                ActionType::PostSmallBlind { amount, .. }
                | ActionType::PostBigBlind { amount, .. }
                | ActionType::PostBlind { amount }
                | ActionType::PostAnte { amount }
                | ActionType::BringsIn { amount } => {
                    pot += amount.amount;
                    *round_invested.entry(&action.player).or_default() += amount.amount;
                }
                ActionType::Call { amount, .. } => {
                    pot += amount.amount;
                    *round_invested.entry(&action.player).or_default() += amount.amount;
                }
                ActionType::Bet { amount, .. } => {
                    let amt = amount.amount;
                    pot += amt;
                    *round_invested.entry(&action.player).or_default() += amt;

                    if action.player == player {
                        let sizing = match action.street {
                            Street::Preflop => &mut preflop,
                            Street::Flop => &mut flop,
                            Street::Turn => &mut turn,
                            Street::River => &mut river,
                            _ => continue,
                        };
                        sizing.total_bets += 1;
                        total_sizing_actions += 1;
                        let bucket = size_bucket(amt, pot_before);
                        *sizing.buckets.entry(bucket).or_default() += 1;
                        if pot_before > 0.0 {
                            sizing.sizes_pct.push(amt / pot_before * 100.0);
                        }
                    }
                }
                ActionType::Raise { to, .. } => {
                    let prev = round_invested.get(action.player.as_str()).copied().unwrap_or(0.0);
                    let increment = to.amount - prev;
                    pot += increment;
                    *round_invested.entry(&action.player).or_default() = to.amount;

                    if action.player == player {
                        let raise_amount = increment;
                        let sizing = match action.street {
                            Street::Preflop => &mut preflop,
                            Street::Flop => &mut flop,
                            Street::Turn => &mut turn,
                            Street::River => &mut river,
                            _ => continue,
                        };
                        sizing.total_bets += 1;
                        total_sizing_actions += 1;
                        let bucket = size_bucket(raise_amount, pot_before);
                        *sizing.buckets.entry(bucket).or_default() += 1;
                        if pot_before > 0.0 {
                            sizing.sizes_pct.push(raise_amount / pot_before * 100.0);
                        }
                    }
                }
                ActionType::UncalledBet { amount } => {
                    pot -= amount.amount;
                }
                _ => {}
            }
        }
    }

    let sizing_json = |name: &str, s: &mut StreetSizing| -> serde_json::Value {
        if s.total_bets == 0 {
            return serde_json::json!({
                "street": name,
                "total_bets_raises": 0,
            });
        }

        s.sizes_pct.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let avg = s.sizes_pct.iter().sum::<f64>() / s.sizes_pct.len() as f64;
        let median = if s.sizes_pct.len() % 2 == 0 {
            let mid = s.sizes_pct.len() / 2;
            (s.sizes_pct[mid - 1] + s.sizes_pct[mid]) / 2.0
        } else {
            s.sizes_pct[s.sizes_pct.len() / 2]
        };

        let pct = |n: u64| -> f64 {
            if s.total_bets > 0 { n as f64 / s.total_bets as f64 * 100.0 } else { 0.0 }
        };

        let bucket_order = [
            "tiny (<30%)", "third (30-40%)", "half (40-55%)",
            "two-thirds (55-72%)", "three-quarters (72-88%)",
            "pot (88-115%)", "overbet (>115%)", "unknown",
        ];
        let distribution: Vec<serde_json::Value> = bucket_order.iter()
            .filter_map(|&b| {
                let count = s.buckets.get(b).copied().unwrap_or(0);
                if count > 0 {
                    Some(serde_json::json!({
                        "size": b,
                        "count": count,
                        "pct": format!("{:.1}", pct(count)),
                    }))
                } else {
                    None
                }
            })
            .collect();

        serde_json::json!({
            "street": name,
            "total_bets_raises": s.total_bets,
            "avg_size_pct_pot": format!("{:.1}%", avg),
            "median_size_pct_pot": format!("{:.1}%", median),
            "distribution": distribution,
        })
    };

    let total_hands = hands.iter()
        .filter(|h| h.players.iter().any(|p| p.name == player && !p.is_sitting_out))
        .count();

    serde_json::json!({
        "player": player,
        "total_hands": total_hands,
        "total_sizing_actions": total_sizing_actions,
        "streets": [
            sizing_json("preflop", &mut preflop),
            sizing_json("flop", &mut flop),
            sizing_json("turn", &mut turn),
            sizing_json("river", &mut river),
        ],
    })
}

// ── get_villain_tendencies ──────────────────────────────────────────────────

pub fn get_villain_tendencies_analysis(hands: &[Hand], hero: &str, villain: &str) -> serde_json::Value {
    #[derive(Default)]
    struct Reactions {
        opps: u64,
        calls: u64,
        folds: u64,
        raises: u64,
    }
    impl Reactions {
        fn to_json(&self) -> serde_json::Value {
            if self.opps == 0 {
                return serde_json::json!({ "opportunities": 0 });
            }
            let pct = |n: u64| -> String { format!("{:.1}", n as f64 / self.opps as f64 * 100.0) };
            serde_json::json!({
                "opportunities": self.opps,
                "call_pct": pct(self.calls),
                "fold_pct": pct(self.folds),
                "raise_pct": pct(self.raises),
            })
        }
    }

    #[derive(Default)]
    struct Initiatives {
        opps: u64,
        bets: u64,
        checks: u64,
    }
    impl Initiatives {
        fn to_json(&self) -> serde_json::Value {
            if self.opps == 0 {
                return serde_json::json!({ "opportunities": 0 });
            }
            let pct = |n: u64| -> String { format!("{:.1}", n as f64 / self.opps as f64 * 100.0) };
            serde_json::json!({
                "opportunities": self.opps,
                "bet_pct": pct(self.bets),
                "check_pct": pct(self.checks),
            })
        }
    }

    let mut vs_flop_bet = Reactions::default();
    let mut vs_turn_bet = Reactions::default();
    let mut vs_river_bet = Reactions::default();
    let mut vs_flop_check = Initiatives::default();
    let mut vs_turn_check = Initiatives::default();
    let mut vs_river_check = Initiatives::default();
    let mut vs_turn_barrel_after_flop_call = Reactions::default();
    let mut vs_river_barrel_after_turn_call = Reactions::default();
    let mut vs_preflop_raise = Reactions::default();
    let mut vs_three_bet = Reactions::default();

    let postflop_streets = [Street::Flop, Street::Turn, Street::River];

    for hand in hands {
        let hero_in = hand.players.iter().any(|p| p.name == hero && !p.is_sitting_out);
        let villain_in = hand.players.iter().any(|p| p.name == villain && !p.is_sitting_out);
        if !hero_in || !villain_in { continue; }

        for &street in &postflop_streets {
            let street_actions: Vec<&crate::types::Action> = hand.actions.iter()
                .filter(|a| a.street == street)
                .collect();
            if street_actions.is_empty() { continue; }

            let mut hero_bet = false;
            let mut hero_checked = false;
            let mut villain_responded = false;

            for action in &street_actions {
                if action.player == hero && !villain_responded {
                    match &action.action_type {
                        ActionType::Bet { .. } | ActionType::Raise { .. } => {
                            hero_bet = true;
                            hero_checked = false;
                        }
                        ActionType::Check => {
                            if !hero_bet { hero_checked = true; }
                        }
                        _ => {}
                    }
                } else if action.player == villain && (hero_bet || hero_checked) && !villain_responded {
                    villain_responded = true;

                    if hero_bet {
                        let reactions = match street {
                            Street::Flop => &mut vs_flop_bet,
                            Street::Turn => &mut vs_turn_bet,
                            Street::River => &mut vs_river_bet,
                            _ => continue,
                        };
                        reactions.opps += 1;
                        match &action.action_type {
                            ActionType::Call { .. } => reactions.calls += 1,
                            ActionType::Fold => reactions.folds += 1,
                            ActionType::Raise { .. } => reactions.raises += 1,
                            _ => { reactions.opps -= 1; }
                        }
                    } else if hero_checked {
                        let initiatives = match street {
                            Street::Flop => &mut vs_flop_check,
                            Street::Turn => &mut vs_turn_check,
                            Street::River => &mut vs_river_check,
                            _ => continue,
                        };
                        initiatives.opps += 1;
                        match &action.action_type {
                            ActionType::Bet { .. } | ActionType::Raise { .. } => initiatives.bets += 1,
                            ActionType::Check => initiatives.checks += 1,
                            _ => { initiatives.opps -= 1; }
                        }
                    }
                }
            }
        }

        // Multi-street sequences
        {
            let villain_called_flop = hand.actions.iter().any(|a| {
                a.player == villain && a.street == Street::Flop && matches!(&a.action_type, ActionType::Call { .. })
            });
            if villain_called_flop {
                let hero_bet_turn = hand.actions.iter().any(|a| {
                    a.player == hero && a.street == Street::Turn && matches!(&a.action_type, ActionType::Bet { .. } | ActionType::Raise { .. })
                });
                if hero_bet_turn {
                    let mut hero_acted = false;
                    for action in &hand.actions {
                        if action.street != Street::Turn { continue; }
                        if action.player == hero {
                            if matches!(&action.action_type, ActionType::Bet { .. } | ActionType::Raise { .. }) {
                                hero_acted = true;
                            }
                        } else if action.player == villain && hero_acted {
                            vs_turn_barrel_after_flop_call.opps += 1;
                            match &action.action_type {
                                ActionType::Call { .. } => vs_turn_barrel_after_flop_call.calls += 1,
                                ActionType::Fold => vs_turn_barrel_after_flop_call.folds += 1,
                                ActionType::Raise { .. } => vs_turn_barrel_after_flop_call.raises += 1,
                                _ => { vs_turn_barrel_after_flop_call.opps -= 1; }
                            }
                            break;
                        }
                    }
                }
            }

            let villain_called_turn = hand.actions.iter().any(|a| {
                a.player == villain && a.street == Street::Turn && matches!(&a.action_type, ActionType::Call { .. })
            });
            if villain_called_turn {
                let hero_bet_river = hand.actions.iter().any(|a| {
                    a.player == hero && a.street == Street::River && matches!(&a.action_type, ActionType::Bet { .. } | ActionType::Raise { .. })
                });
                if hero_bet_river {
                    let mut hero_acted = false;
                    for action in &hand.actions {
                        if action.street != Street::River { continue; }
                        if action.player == hero {
                            if matches!(&action.action_type, ActionType::Bet { .. } | ActionType::Raise { .. }) {
                                hero_acted = true;
                            }
                        } else if action.player == villain && hero_acted {
                            vs_river_barrel_after_turn_call.opps += 1;
                            match &action.action_type {
                                ActionType::Call { .. } => vs_river_barrel_after_turn_call.calls += 1,
                                ActionType::Fold => vs_river_barrel_after_turn_call.folds += 1,
                                ActionType::Raise { .. } => vs_river_barrel_after_turn_call.raises += 1,
                                _ => { vs_river_barrel_after_turn_call.opps -= 1; }
                            }
                            break;
                        }
                    }
                }
            }
        }

        // Preflop
        {
            let mut hero_raised = false;
            let mut raise_count = 0u32;
            let mut villain_preflop_responded = false;
            for action in &hand.actions {
                if action.street != Street::Preflop { continue; }
                if action.player == hero {
                    if matches!(&action.action_type, ActionType::Raise { .. }) {
                        hero_raised = true;
                        raise_count += 1;
                    }
                } else if action.player == villain && hero_raised && !villain_preflop_responded {
                    villain_preflop_responded = true;
                    let target = if raise_count >= 2 { &mut vs_three_bet } else { &mut vs_preflop_raise };
                    target.opps += 1;
                    match &action.action_type {
                        ActionType::Call { .. } => target.calls += 1,
                        ActionType::Fold => target.folds += 1,
                        ActionType::Raise { .. } => target.raises += 1,
                        _ => { target.opps -= 1; }
                    }
                }
            }
        }
    }

    let total_hands = hands.iter()
        .filter(|h| {
            h.players.iter().any(|p| p.name == hero && !p.is_sitting_out)
            && h.players.iter().any(|p| p.name == villain && !p.is_sitting_out)
        })
        .count();

    serde_json::json!({
        "villain": villain,
        "hero": hero,
        "total_hands": total_hands,
        "preflop": {
            "vs_hero_raise": vs_preflop_raise.to_json(),
            "vs_hero_3bet": vs_three_bet.to_json(),
        },
        "flop": {
            "vs_hero_bet": vs_flop_bet.to_json(),
            "vs_hero_check": vs_flop_check.to_json(),
        },
        "turn": {
            "vs_hero_bet": vs_turn_bet.to_json(),
            "vs_hero_check": vs_turn_check.to_json(),
            "vs_barrel_after_calling_flop": vs_turn_barrel_after_flop_call.to_json(),
        },
        "river": {
            "vs_hero_bet": vs_river_bet.to_json(),
            "vs_hero_check": vs_river_check.to_json(),
            "vs_barrel_after_calling_turn": vs_river_barrel_after_turn_call.to_json(),
        },
    })
}

// ── get_board_stats ─────────────────────────────────────────────────────────

fn suit_texture(board: &[Card]) -> &'static str {
    if board.len() < 3 { return "unknown"; }
    let s1 = board[0].suit;
    let s2 = board[1].suit;
    let s3 = board[2].suit;
    if s1 == s2 && s2 == s3 { "monotone" }
    else if s1 != s2 && s2 != s3 && s1 != s3 { "rainbow" }
    else { "two-tone" }
}

fn is_paired(board: &[Card]) -> bool {
    if board.len() < 3 { return false; }
    let r = [rank_order(board[0].rank), rank_order(board[1].rank), rank_order(board[2].rank)];
    r[0] == r[1] || r[1] == r[2] || r[0] == r[2]
}

fn is_connected(board: &[Card]) -> bool {
    if board.len() < 3 { return false; }
    let mut r = [rank_order(board[0].rank), rank_order(board[1].rank), rank_order(board[2].rank)];
    r.sort();
    let spread = r[2] - r[0];
    spread <= 4
}

fn highness(board: &[Card]) -> &'static str {
    if board.len() < 3 { return "unknown"; }
    let high_cards = board.iter().take(3)
        .filter(|c| rank_order(c.rank) >= 10)
        .count();
    if high_cards >= 2 { "high" }
    else if high_cards == 0 { "low" }
    else { "mid" }
}

fn wetness(board: &[Card]) -> &'static str {
    if board.len() < 3 { return "unknown"; }
    let flush_draw = {
        let s1 = board[0].suit;
        let s2 = board[1].suit;
        let s3 = board[2].suit;
        s1 == s2 || s2 == s3 || s1 == s3
    };
    let straight_draw = is_connected(board);
    if flush_draw && straight_draw { "very wet" }
    else if flush_draw || straight_draw { "wet" }
    else { "dry" }
}

pub fn get_board_stats_analysis(hands: &[Hand], hero: &str) -> serde_json::Value {
    let texture_names = [
        "monotone", "two-tone", "rainbow",
        "paired", "connected",
        "high", "mid", "low",
        "dry", "wet", "very wet",
    ];

    struct TextureBucket {
        hands: u64,
        wins: u64,
        profit_bb: f64,
        cbet_opps: u64,
        cbet_count: u64,
    }
    impl TextureBucket {
        fn new() -> Self {
            Self { hands: 0, wins: 0, profit_bb: 0.0, cbet_opps: 0, cbet_count: 0 }
        }
    }

    let mut buckets: HashMap<&str, TextureBucket> = HashMap::new();
    for name in &texture_names {
        buckets.insert(name, TextureBucket::new());
    }
    let mut total_flop_hands = 0u64;

    for hand in hands {
        let in_hand = hand.players.iter().any(|p| p.name == hero && !p.is_sitting_out);
        if !in_hand { continue; }
        if hand.board.len() < 3 { continue; }

        let hero_saw_flop = hand.actions.iter().any(|a| {
            a.player == hero && a.street == Street::Flop
        });
        if !hero_saw_flop { continue; }

        total_flop_hands += 1;

        let bb = stats::big_blind_size(hand);
        let profit = stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
        let profit_bb_val = if bb > 0.0 { profit / bb } else { 0.0 };
        let won = profit > 0.0;

        let hero_was_pfr = hand.actions.iter().any(|a| {
            a.player == hero && a.street == Street::Preflop
                && matches!(&a.action_type, ActionType::Raise { .. })
        });

        let hero_cbet = hero_was_pfr && hand.actions.iter().any(|a| {
            a.player == hero && a.street == Street::Flop
                && matches!(&a.action_type, ActionType::Bet { .. })
        });

        let flop = &hand.board[..3];
        let suit_tex = suit_texture(flop);
        let paired = is_paired(flop);
        let connected = is_connected(flop);
        let high_tex = highness(flop);
        let wet_tex = wetness(flop);

        let mut apply = |name: &'static str| {
            let b = buckets.get_mut(name).unwrap();
            b.hands += 1;
            if won { b.wins += 1; }
            b.profit_bb += profit_bb_val;
            if hero_was_pfr {
                b.cbet_opps += 1;
                if hero_cbet { b.cbet_count += 1; }
            }
        };

        apply(suit_tex);
        if paired { apply("paired"); }
        if connected { apply("connected"); }
        apply(high_tex);
        apply(wet_tex);
    }

    let pct = |n: u64, d: u64| -> f64 {
        if d > 0 { n as f64 / d as f64 * 100.0 } else { 0.0 }
    };

    let mut textures: Vec<serde_json::Value> = texture_names.iter()
        .filter_map(|&name| {
            let b = buckets.get(name).unwrap();
            if b.hands == 0 { return None; }
            let winrate = if b.hands > 0 {
                b.profit_bb / b.hands as f64 * 100.0
            } else { 0.0 };
            Some(serde_json::json!({
                "texture": name,
                "hands": b.hands,
                "win_pct": format!("{:.1}", pct(b.wins, b.hands)),
                "winrate_bb100": format!("{:.1}", winrate),
                "profit_bb": format!("{:.1}", b.profit_bb),
                "cbet_pct": format!("{:.1}", pct(b.cbet_count, b.cbet_opps)),
                "cbet_opportunities": b.cbet_opps,
            }))
        })
        .collect();

    textures.sort_by(|a, b| {
        let ha = a["hands"].as_u64().unwrap_or(0);
        let hb = b["hands"].as_u64().unwrap_or(0);
        hb.cmp(&ha)
    });

    serde_json::json!({
        "player": hero,
        "total_hands_with_flop": total_flop_hands,
        "textures": textures,
    })
}

// ── get_range_analysis ──────────────────────────────────────────────────────

pub fn hand_category(combo: &str) -> &'static str {
    let chars: Vec<char> = combo.chars().collect();
    if chars.len() < 2 { return "other"; }

    let is_pair = chars.len() == 2;
    let is_suited = chars.last() == Some(&'s');
    let is_offsuit = chars.last() == Some(&'o');

    if is_pair { return "pocket_pairs"; }

    let broadways = ['A', 'K', 'Q', 'J', 'T'];
    let c1_broadway = broadways.contains(&chars[0]);
    let c2_broadway = broadways.contains(&chars[1]);

    if c1_broadway && c2_broadway {
        if is_suited { return "suited_broadways"; }
        else { return "offsuit_broadways"; }
    }

    let rank_val = |c: char| -> Option<u8> {
        match c {
            '2' => Some(2), '3' => Some(3), '4' => Some(4), '5' => Some(5),
            '6' => Some(6), '7' => Some(7), '8' => Some(8), '9' => Some(9),
            'T' => Some(10), 'J' => Some(11), 'Q' => Some(12), 'K' => Some(13),
            'A' => Some(14), _ => None,
        }
    };

    let gap = match (rank_val(chars[0]), rank_val(chars[1])) {
        (Some(a), Some(b)) => (a as i8 - b as i8).unsigned_abs(),
        _ => 99,
    };

    if is_suited {
        if gap <= 2 { "suited_connectors" }
        else if c1_broadway || c2_broadway { "suited_aces_kings" }
        else { "suited_other" }
    } else if is_offsuit {
        if gap <= 2 { "offsuit_connectors" }
        else { "offsuit_other" }
    } else {
        "other"
    }
}

pub fn get_range_analysis_data(hands: &[Hand], hero: &str, position: &str) -> serde_json::Value {
    #[derive(Default)]
    struct ComboData {
        total: u64,
        open: u64,
        three_bet: u64,
        call: u64,
        fold: u64,
        limp: u64,
        wins: u64,
        profit_bb: f64,
    }

    let mut combos: HashMap<String, ComboData> = HashMap::new();
    let mut total_dealt = 0u64;
    let mut total_with_cards = 0u64;

    for hand in hands {
        let in_hand = hand.players.iter().any(|p| p.name == hero && !p.is_sitting_out);
        if !in_hand { continue; }
        total_dealt += 1;

        if hand.hero_cards.len() != 2 { continue; }

        let label = match combo_label(&hand.hero_cards) {
            Some(l) => l,
            None => continue,
        };
        total_with_cards += 1;

        let bb = stats::big_blind_size(hand);
        let profit = stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
        let profit_bb_val = if bb > 0.0 { profit / bb } else { 0.0 };

        let data = combos.entry(label).or_default();
        data.total += 1;
        if profit > 0.0 { data.wins += 1; }
        data.profit_bb += profit_bb_val;

        let mut raises_before_hero = 0u32;
        let mut hero_action_found = false;

        for action in &hand.actions {
            if action.street != Street::Preflop { continue; }

            if action.player == hero {
                if hero_action_found { continue; }
                match &action.action_type {
                    ActionType::PostSmallBlind { .. }
                    | ActionType::PostBigBlind { .. }
                    | ActionType::PostAnte { .. }
                    | ActionType::PostBlind { .. } => {}
                    ActionType::Raise { .. } => {
                        hero_action_found = true;
                        if raises_before_hero == 0 {
                            data.open += 1;
                        } else {
                            data.three_bet += 1;
                        }
                    }
                    ActionType::Call { .. } => {
                        hero_action_found = true;
                        if raises_before_hero == 0 {
                            data.limp += 1;
                        } else {
                            data.call += 1;
                        }
                    }
                    ActionType::Fold => {
                        hero_action_found = true;
                        data.fold += 1;
                    }
                    _ => {}
                }
            } else {
                if !hero_action_found {
                    if let ActionType::Raise { .. } = &action.action_type {
                        raises_before_hero += 1;
                    }
                }
            }
        }
    }

    if combos.is_empty() {
        return serde_json::json!({
            "position": position,
            "total_hands": total_dealt,
            "message": "No hands with visible hole cards found.",
        });
    }

    let pct = |n: u64, d: u64| -> f64 {
        if d > 0 { n as f64 / d as f64 * 100.0 } else { 0.0 }
    };

    let mut combo_results: Vec<serde_json::Value> = combos.iter()
        .map(|(label, d)| {
            serde_json::json!({
                "combo": label,
                "category": hand_category(label),
                "count": d.total,
                "open_pct": format!("{:.1}", pct(d.open, d.total)),
                "three_bet_pct": format!("{:.1}", pct(d.three_bet, d.total)),
                "call_pct": format!("{:.1}", pct(d.call, d.total)),
                "fold_pct": format!("{:.1}", pct(d.fold, d.total)),
                "limp_pct": format!("{:.1}", pct(d.limp, d.total)),
                "win_pct": format!("{:.1}", pct(d.wins, d.total)),
                "profit_bb": format!("{:.1}", d.profit_bb),
            })
        })
        .collect();

    combo_results.sort_by(|a, b| {
        let ca = a["count"].as_u64().unwrap_or(0);
        let cb = b["count"].as_u64().unwrap_or(0);
        cb.cmp(&ca)
    });

    let category_order = [
        "pocket_pairs", "suited_broadways", "offsuit_broadways",
        "suited_connectors", "offsuit_connectors",
        "suited_aces_kings", "suited_other", "offsuit_other",
    ];

    let mut categories: Vec<serde_json::Value> = category_order.iter()
        .filter_map(|&cat| {
            let cat_combos: Vec<&serde_json::Value> = combo_results.iter()
                .filter(|c| c["category"].as_str() == Some(cat))
                .collect();
            if cat_combos.is_empty() { return None; }

            let total: u64 = cat_combos.iter().map(|c| c["count"].as_u64().unwrap_or(0)).sum();
            let unique = cat_combos.len();

            Some(serde_json::json!({
                "category": cat,
                "unique_combos": unique,
                "total_hands": total,
                "pct_of_range": format!("{:.1}", pct(total, total_with_cards)),
            }))
        })
        .collect();

    categories.sort_by(|a, b| {
        let ca = a["total_hands"].as_u64().unwrap_or(0);
        let cb = b["total_hands"].as_u64().unwrap_or(0);
        cb.cmp(&ca)
    });

    serde_json::json!({
        "position": position,
        "player": hero,
        "total_hands_dealt": total_dealt,
        "total_hands_with_cards": total_with_cards,
        "unique_combos_seen": combos.len(),
        "categories": categories,
        "combos": combo_results,
    })
}

// ── get_trends ──────────────────────────────────────────────────────────────

pub fn get_trends_analysis(
    mut hands: Vec<Hand>,
    hero: &str,
    period: &str,
) -> serde_json::Value {
    if hands.is_empty() {
        return serde_json::json!({
            "periods": [],
            "total_hands": 0,
        });
    }

    hands.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let bucket_key = |ts: &str| -> String {
        let date_part = ts.split(' ').next().unwrap_or(ts);
        match period {
            "day" => date_part.to_string(),
            "month" => {
                let parts: Vec<&str> = date_part.split('/').collect();
                if parts.len() >= 2 {
                    format!("{}/{}", parts[0], parts[1])
                } else {
                    date_part.to_string()
                }
            }
            "week" | _ => {
                let parts: Vec<&str> = date_part.split('/').collect();
                if parts.len() == 3 {
                    let year: i32 = parts[0].parse().unwrap_or(0);
                    let month: u32 = parts[1].parse().unwrap_or(1);
                    let day: u32 = parts[2].parse().unwrap_or(1);
                    let days = days_from_ymd(year, month, day);
                    let dow = ((days % 7) + 7) % 7;
                    let monday = days - dow;
                    let (my, mm, md) = ymd_from_days(monday);
                    format!("{:04}/{:02}/{:02}", my, mm, md)
                } else {
                    date_part.to_string()
                }
            }
        }
    };

    let mut buckets: Vec<(String, Vec<&Hand>)> = Vec::new();
    for hand in &hands {
        let key = bucket_key(&hand.timestamp);
        if let Some(last) = buckets.last_mut() {
            if last.0 == key {
                last.1.push(hand);
                continue;
            }
        }
        buckets.push((key, vec![hand]));
    }

    let mut cumulative_profit = 0.0f64;
    let mut periods = Vec::new();
    for (key, bucket_hands) in &buckets {
        let owned: Vec<Hand> = bucket_hands.iter().map(|h| (*h).clone()).collect();
        let s = stats::calculate_stats(&owned, hero);

        let mut period_profit = 0.0f64;
        for hand in bucket_hands {
            let invested = stats::hero_invested(hand, hero);
            let collected = stats::hero_collected(hand, hero);
            period_profit += collected - invested;
        }
        cumulative_profit += period_profit;

        let label = match period {
            "week" => format!("week of {}", key),
            _ => key.clone(),
        };

        periods.push(serde_json::json!({
            "period": label,
            "hands": s.hands_played,
            "vpip": format!("{:.1}", s.vpip),
            "pfr": format!("{:.1}", s.pfr),
            "three_bet_pct": format!("{:.1}", s.three_bet_pct),
            "aggression_factor": format!("{:.2}", s.aggression_factor),
            "winrate_bb100": format!("{:.1}", s.winrate_bb100),
            "profit": format!("{:.2}", period_profit),
            "cumulative_profit": format!("{:.2}", cumulative_profit),
            "went_to_showdown_pct": format!("{:.1}", s.went_to_showdown_pct),
            "won_at_showdown_pct": format!("{:.1}", s.won_at_showdown_pct),
            "cbet_flop": format!("{:.1}", s.cbet_flop),
            "wwsf": format!("{:.1}", s.wwsf),
        }));
    }

    serde_json::json!({
        "period_type": period,
        "total_hands": hands.len(),
        "total_periods": periods.len(),
        "periods": periods,
    })
}
