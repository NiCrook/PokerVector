use std::collections::HashMap;

use crate::types::*;

/// Tracks pot size and per-player investment for accurate sizing calculations.
pub(super) struct PotTracker {
    /// Total pot entering the current street
    pub(super) pot_at_street_start: f64,
    /// Running pot within the current street
    pub(super) current_pot: f64,
    /// Hero's total investment across all streets
    pub(super) hero_invested: f64,
    /// Per-player investment in the current betting round (excludes antes)
    current_round: HashMap<String, f64>,
    /// The current bet/raise amount to call on this street
    pub(super) current_bet: f64,
}

impl PotTracker {
    pub(super) fn new() -> Self {
        Self {
            pot_at_street_start: 0.0,
            current_pot: 0.0,
            hero_invested: 0.0,
            current_round: HashMap::new(),
            current_bet: 0.0,
        }
    }

    pub(super) fn new_street(&mut self) {
        self.pot_at_street_start = self.current_pot;
        self.current_round.clear();
        self.current_bet = 0.0;
    }

    pub(super) fn process_action(&mut self, action: &Action, hero: &str) {
        let is_hero = action.player == hero;
        match &action.action_type {
            ActionType::PostSmallBlind { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::PostBigBlind { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                self.current_bet = amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::PostBlind { amount } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::PostAnte { amount } => {
                // Antes go to pot and hero_invested but NOT current_round
                let amt = amount.amount;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::BringsIn { amount } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                self.current_bet = amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::Call { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::Bet { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                self.current_bet = amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::Raise { to, .. } => {
                let prev = self
                    .current_round
                    .get(&action.player)
                    .copied()
                    .unwrap_or(0.0);
                let increment = to.amount - prev;
                *self.current_round.entry(action.player.clone()).or_default() = to.amount;
                self.current_pot += increment;
                self.current_bet = to.amount;
                if is_hero {
                    self.hero_invested += increment;
                }
            }
            ActionType::UncalledBet { amount } => {
                let amt = amount.amount;
                self.current_pot -= amt;
                if is_hero {
                    self.hero_invested -= amt;
                }
            }
            _ => {}
        }
    }
}
