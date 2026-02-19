mod calculate;
mod cbet;
mod helpers;
mod limp;
mod postflop;
mod preflop;
mod steal;
mod villains;

#[cfg(test)]
mod test_helpers;

pub use calculate::calculate_stats;
pub use villains::list_villains;

use crate::types::*;

pub use self::types::{PlayerStats, PositionStats, VillainSummary};

mod types {
    use std::collections::HashMap;

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct PlayerStats {
        pub hands_played: u64,
        pub vpip: f64,
        pub pfr: f64,
        pub three_bet_pct: f64,
        pub fold_to_three_bet: f64,
        pub aggression_factor: f64,
        pub winrate_bb100: f64,
        pub went_to_showdown_pct: f64,
        pub won_at_showdown_pct: f64,
        pub cbet_flop: f64,
        pub cbet_turn: f64,
        pub fold_to_cbet_flop: f64,
        pub fold_to_cbet_turn: f64,
        pub steal_pct: f64,
        pub fold_to_steal_bb: f64,
        pub fold_to_steal_sb: f64,
        pub limp_pct: f64,
        pub limp_call: f64,
        pub limp_fold: f64,
        pub limp_raise: f64,
        pub donk_bet_pct: f64,
        pub float_pct: f64,
        pub check_raise_pct: f64,
        pub probe_bet_pct: f64,
        pub squeeze_pct: f64,
        pub cold_call_pct: f64,
        pub wwsf: f64,
        pub overbet_pct: f64,
        pub positions: Option<HashMap<String, PositionStats>>,
    }

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct PositionStats {
        pub hands: u64,
        pub vpip: f64,
        pub pfr: f64,
    }

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct VillainSummary {
        pub name: String,
        pub hands: u64,
        pub vpip: f64,
        pub pfr: f64,
        pub aggression_factor: f64,
        pub three_bet_pct: f64,
        pub fold_to_three_bet: f64,
        pub cbet_flop: f64,
        pub fold_to_cbet_flop: f64,
        pub steal_pct: f64,
        pub wwsf: f64,
    }
}

/// Classify a hand's pot type based on preflop action.
pub fn classify_pot_type(hand: &Hand) -> &'static str {
    let mut voluntary_raises = 0u32;
    let mut voluntary_calls = 0u32;

    for action in &hand.actions {
        if action.street != Street::Preflop {
            continue;
        }
        match &action.action_type {
            ActionType::Raise { .. } | ActionType::Bet { .. } => {
                voluntary_raises += 1;
            }
            ActionType::Call { .. } => {
                voluntary_calls += 1;
            }
            _ => {}
        }
    }

    match voluntary_raises {
        0 if voluntary_calls == 0 => "walk",
        0 => "limp",
        1 => "SRP",
        2 => "3bet",
        _ => "4bet",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::test_helpers::*;

    #[test]
    fn test_classify_walk() {
        let mut hand = base_hand();
        hand.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];
        assert_eq!(classify_pot_type(&hand), "walk");
    }

    #[test]
    fn test_classify_limp() {
        let mut hand = base_hand();
        hand.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Check, street: Street::Preflop },
        ];
        assert_eq!(classify_pot_type(&hand), "limp");
    }

    #[test]
    fn test_classify_srp() {
        let mut hand = base_hand();
        hand.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(0.05), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];
        assert_eq!(classify_pot_type(&hand), "SRP");
    }

    #[test]
    fn test_classify_3bet() {
        let mut hand = base_hand();
        hand.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.06), to: make_money(0.18), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(0.12), all_in: false }, street: Street::Preflop },
        ];
        assert_eq!(classify_pot_type(&hand), "3bet");
    }

    #[test]
    fn test_classify_4bet() {
        let mut hand = base_hand();
        hand.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.06), to: make_money(0.18), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.18), to: make_money(0.50), all_in: false }, street: Street::Preflop },
        ];
        assert_eq!(classify_pot_type(&hand), "4bet");
    }
}
