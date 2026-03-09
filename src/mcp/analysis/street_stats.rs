use crate::types::{ActionType, Hand, Street};

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
            Self {
                hands_seen: 0,
                bets: 0,
                raises: 0,
                calls: 0,
                checks: 0,
                folds: 0,
            }
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
        let in_hand = hand
            .players
            .iter()
            .any(|p| p.name == player && !p.is_sitting_out);
        if !in_hand {
            continue;
        }

        for &street in &streets_of_interest {
            let player_acted = hand
                .actions
                .iter()
                .any(|a| a.player == player && a.street == street);
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
            if action.player != player {
                continue;
            }
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
        if den > 0 {
            num as f64 / den as f64 * 100.0
        } else {
            0.0
        }
    };

    let street_json = |name: &str, c: &StreetCounts| -> serde_json::Value {
        let total = c.total_actions();
        let agg = c.aggressive_actions();
        let af = if c.calls > 0 {
            agg as f64 / c.calls as f64
        } else if agg > 0 {
            f64::INFINITY
        } else {
            0.0
        };
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
        "streets": [
            street_json("flop", &flop),
            street_json("turn", &turn),
            street_json("river", &river),
        ],
    })
}
