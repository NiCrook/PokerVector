use std::collections::HashMap;

use crate::types::{ActionType, Hand, Street};

pub fn get_sizing_profile_analysis(hands: &[Hand], player: &str) -> serde_json::Value {
    fn size_bucket(bet_amount: f64, pot_before: f64) -> &'static str {
        if pot_before <= 0.0 {
            return "unknown";
        }
        let ratio = bet_amount / pot_before;
        if ratio < 0.30 {
            "tiny (<30%)"
        } else if ratio < 0.40 {
            "third (30-40%)"
        } else if ratio < 0.55 {
            "half (40-55%)"
        } else if ratio < 0.72 {
            "two-thirds (55-72%)"
        } else if ratio < 0.88 {
            "three-quarters (72-88%)"
        } else if ratio < 1.15 {
            "pot (88-115%)"
        } else {
            "overbet (>115%)"
        }
    }

    struct StreetSizing {
        total_bets: u64,
        buckets: HashMap<&'static str, u64>,
        sizes_pct: Vec<f64>,
    }

    impl StreetSizing {
        fn new() -> Self {
            Self {
                total_bets: 0,
                buckets: HashMap::new(),
                sizes_pct: Vec::new(),
            }
        }
    }

    let mut preflop = StreetSizing::new();
    let mut flop = StreetSizing::new();
    let mut turn = StreetSizing::new();
    let mut river = StreetSizing::new();
    let mut total_sizing_actions = 0u64;

    for hand in hands {
        let in_hand = hand
            .players
            .iter()
            .any(|p| p.name == player && !p.is_sitting_out);
        if !in_hand {
            continue;
        }

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
                    let prev = round_invested
                        .get(action.player.as_str())
                        .copied()
                        .unwrap_or(0.0);
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

        s.sizes_pct
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let avg = s.sizes_pct.iter().sum::<f64>() / s.sizes_pct.len() as f64;
        let median = if s.sizes_pct.len() % 2 == 0 {
            let mid = s.sizes_pct.len() / 2;
            (s.sizes_pct[mid - 1] + s.sizes_pct[mid]) / 2.0
        } else {
            s.sizes_pct[s.sizes_pct.len() / 2]
        };

        let pct = |n: u64| -> f64 {
            if s.total_bets > 0 {
                n as f64 / s.total_bets as f64 * 100.0
            } else {
                0.0
            }
        };

        let bucket_order = [
            "tiny (<30%)",
            "third (30-40%)",
            "half (40-55%)",
            "two-thirds (55-72%)",
            "three-quarters (72-88%)",
            "pot (88-115%)",
            "overbet (>115%)",
            "unknown",
        ];
        let distribution: Vec<serde_json::Value> = bucket_order
            .iter()
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

    let total_hands = hands
        .iter()
        .filter(|h| {
            h.players
                .iter()
                .any(|p| p.name == player && !p.is_sitting_out)
        })
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
