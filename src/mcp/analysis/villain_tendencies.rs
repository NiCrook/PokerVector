use crate::types::{ActionType, Hand, Street};

pub fn get_villain_tendencies_analysis(
    hands: &[Hand],
    hero: &str,
    villain: &str,
) -> serde_json::Value {
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
        let hero_in = hand
            .players
            .iter()
            .any(|p| p.name == hero && !p.is_sitting_out);
        let villain_in = hand
            .players
            .iter()
            .any(|p| p.name == villain && !p.is_sitting_out);
        if !hero_in || !villain_in {
            continue;
        }

        for &street in &postflop_streets {
            let street_actions: Vec<&crate::types::Action> =
                hand.actions.iter().filter(|a| a.street == street).collect();
            if street_actions.is_empty() {
                continue;
            }

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
                            if !hero_bet {
                                hero_checked = true;
                            }
                        }
                        _ => {}
                    }
                } else if action.player == villain
                    && (hero_bet || hero_checked)
                    && !villain_responded
                {
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
                            _ => {
                                reactions.opps -= 1;
                            }
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
                            ActionType::Bet { .. } | ActionType::Raise { .. } => {
                                initiatives.bets += 1
                            }
                            ActionType::Check => initiatives.checks += 1,
                            _ => {
                                initiatives.opps -= 1;
                            }
                        }
                    }
                }
            }
        }

        // Multi-street sequences
        {
            let villain_called_flop = hand.actions.iter().any(|a| {
                a.player == villain
                    && a.street == Street::Flop
                    && matches!(&a.action_type, ActionType::Call { .. })
            });
            if villain_called_flop {
                let hero_bet_turn = hand.actions.iter().any(|a| {
                    a.player == hero
                        && a.street == Street::Turn
                        && matches!(
                            &a.action_type,
                            ActionType::Bet { .. } | ActionType::Raise { .. }
                        )
                });
                if hero_bet_turn {
                    let mut hero_acted = false;
                    for action in &hand.actions {
                        if action.street != Street::Turn {
                            continue;
                        }
                        if action.player == hero {
                            if matches!(
                                &action.action_type,
                                ActionType::Bet { .. } | ActionType::Raise { .. }
                            ) {
                                hero_acted = true;
                            }
                        } else if action.player == villain && hero_acted {
                            vs_turn_barrel_after_flop_call.opps += 1;
                            match &action.action_type {
                                ActionType::Call { .. } => {
                                    vs_turn_barrel_after_flop_call.calls += 1
                                }
                                ActionType::Fold => vs_turn_barrel_after_flop_call.folds += 1,
                                ActionType::Raise { .. } => {
                                    vs_turn_barrel_after_flop_call.raises += 1
                                }
                                _ => {
                                    vs_turn_barrel_after_flop_call.opps -= 1;
                                }
                            }
                            break;
                        }
                    }
                }
            }

            let villain_called_turn = hand.actions.iter().any(|a| {
                a.player == villain
                    && a.street == Street::Turn
                    && matches!(&a.action_type, ActionType::Call { .. })
            });
            if villain_called_turn {
                let hero_bet_river = hand.actions.iter().any(|a| {
                    a.player == hero
                        && a.street == Street::River
                        && matches!(
                            &a.action_type,
                            ActionType::Bet { .. } | ActionType::Raise { .. }
                        )
                });
                if hero_bet_river {
                    let mut hero_acted = false;
                    for action in &hand.actions {
                        if action.street != Street::River {
                            continue;
                        }
                        if action.player == hero {
                            if matches!(
                                &action.action_type,
                                ActionType::Bet { .. } | ActionType::Raise { .. }
                            ) {
                                hero_acted = true;
                            }
                        } else if action.player == villain && hero_acted {
                            vs_river_barrel_after_turn_call.opps += 1;
                            match &action.action_type {
                                ActionType::Call { .. } => {
                                    vs_river_barrel_after_turn_call.calls += 1
                                }
                                ActionType::Fold => vs_river_barrel_after_turn_call.folds += 1,
                                ActionType::Raise { .. } => {
                                    vs_river_barrel_after_turn_call.raises += 1
                                }
                                _ => {
                                    vs_river_barrel_after_turn_call.opps -= 1;
                                }
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
                if action.street != Street::Preflop {
                    continue;
                }
                if action.player == hero {
                    if matches!(&action.action_type, ActionType::Raise { .. }) {
                        hero_raised = true;
                        raise_count += 1;
                    }
                } else if action.player == villain && hero_raised && !villain_preflop_responded {
                    villain_preflop_responded = true;
                    let target = if raise_count >= 2 {
                        &mut vs_three_bet
                    } else {
                        &mut vs_preflop_raise
                    };
                    target.opps += 1;
                    match &action.action_type {
                        ActionType::Call { .. } => target.calls += 1,
                        ActionType::Fold => target.folds += 1,
                        ActionType::Raise { .. } => target.raises += 1,
                        _ => {
                            target.opps -= 1;
                        }
                    }
                }
            }
        }
    }

    let total_hands = hands
        .iter()
        .filter(|h| {
            h.players
                .iter()
                .any(|p| p.name == hero && !p.is_sitting_out)
                && h.players
                    .iter()
                    .any(|p| p.name == villain && !p.is_sitting_out)
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
