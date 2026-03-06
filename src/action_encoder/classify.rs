use std::collections::HashMap;

use crate::types::*;

use super::pot_tracker::PotTracker;

#[derive(Debug)]
pub(super) enum ActionLabel {
    Open(f64),
    Bet(f64),
    Raise(f64),
    ThreeBet(f64),
    FourBet(f64),
    FiveBetPlus(f64),
    CBet(f64),
    CallBB(f64),
    CallPot(f64),
    Check,
    Fold,
    BringIn(f64),
}

#[derive(Debug)]
pub(super) struct ClassifiedAction {
    pub(super) label: ActionLabel,
    pub(super) all_in: bool,
    pub(super) alias: String,
}

/// Format a number with given decimal places, trimming trailing zeros and dot.
fn fmt_trimmed(val: f64, decimals: usize) -> String {
    let s = format!("{:.prec$}", val, prec = decimals);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Classify a single action into an ActionLabel with context.
/// Returns None for actions that should not be encoded (blinds, antes, etc.).
pub(super) fn classify_action(
    action: &Action,
    alias_map: &HashMap<String, String>,
    pot_tracker: &PotTracker,
    bb: f64,
    is_preflop: bool,
    is_stud: bool,
    raise_count: &mut u32,
    has_bet_on_street: &mut bool,
    preflop_aggressor: &Option<String>,
) -> Option<ClassifiedAction> {
    let alias = alias_map
        .get(&action.player)
        .cloned()
        .unwrap_or_else(|| action.player.clone());

    match &action.action_type {
        ActionType::Fold => Some(ClassifiedAction {
            label: ActionLabel::Fold,
            all_in: false,
            alias,
        }),
        ActionType::Check => Some(ClassifiedAction {
            label: ActionLabel::Check,
            all_in: false,
            alias,
        }),
        ActionType::Call { amount, all_in } => {
            if is_preflop && !is_stud {
                let size = amount.amount / bb;
                Some(ClassifiedAction {
                    label: ActionLabel::CallBB(size),
                    all_in: *all_in,
                    alias,
                })
            } else {
                let size = if pot_tracker.pot_at_street_start > 0.0 {
                    amount.amount / pot_tracker.pot_at_street_start
                } else {
                    amount.amount / bb
                };
                Some(ClassifiedAction {
                    label: ActionLabel::CallPot(size),
                    all_in: *all_in,
                    alias,
                })
            }
        }
        ActionType::Bet { amount, all_in } => {
            *has_bet_on_street = true;
            let pot_frac = if pot_tracker.pot_at_street_start > 0.0 {
                amount.amount / pot_tracker.pot_at_street_start
            } else {
                amount.amount / bb
            };

            // C-bet detection: preflop aggressor makes first bet on postflop street
            let is_cbet = !is_preflop
                && !is_stud
                && preflop_aggressor
                    .as_ref()
                    .map(|pa| pa == &action.player)
                    .unwrap_or(false);

            let label = if is_cbet {
                ActionLabel::CBet(pot_frac)
            } else {
                ActionLabel::Bet(pot_frac)
            };
            Some(ClassifiedAction {
                label,
                all_in: *all_in,
                alias,
            })
        }
        ActionType::Raise { to, all_in, .. } => {
            *raise_count += 1;

            if is_preflop && !is_stud {
                let size_bb = to.amount / bb;
                let label = match *raise_count {
                    1 => ActionLabel::Open(size_bb),
                    2 => ActionLabel::ThreeBet(size_bb),
                    3 => ActionLabel::FourBet(size_bb),
                    _ => ActionLabel::FiveBetPlus(size_bb),
                };
                Some(ClassifiedAction {
                    label,
                    all_in: *all_in,
                    alias,
                })
            } else {
                // Postflop or stud: multiplier of current bet
                let multiplier = if pot_tracker.current_bet > 0.0 {
                    to.amount / pot_tracker.current_bet
                } else {
                    to.amount / bb
                };
                Some(ClassifiedAction {
                    label: ActionLabel::Raise(multiplier),
                    all_in: *all_in,
                    alias,
                })
            }
        }
        ActionType::BringsIn { amount } => {
            let size_bb = amount.amount / bb;
            Some(ClassifiedAction {
                label: ActionLabel::BringIn(size_bb),
                all_in: false,
                alias,
            })
        }
        // Skip all other action types (blinds, antes, uncalled bets, shows, etc.)
        _ => None,
    }
}

/// Format a ClassifiedAction into a token string.
pub(super) fn format_classified_action(ca: &ClassifiedAction) -> String {
    let ai_suffix = if ca.all_in { "_AI" } else { "" };

    match &ca.label {
        ActionLabel::Open(bb) => {
            format!("{}_OPEN{}({}bb)", ca.alias, ai_suffix, fmt_trimmed(*bb, 1))
        }
        ActionLabel::ThreeBet(bb) => {
            format!("{}_3BET{}({}bb)", ca.alias, ai_suffix, fmt_trimmed(*bb, 1))
        }
        ActionLabel::FourBet(bb) => {
            format!("{}_4BET{}({}bb)", ca.alias, ai_suffix, fmt_trimmed(*bb, 1))
        }
        ActionLabel::FiveBetPlus(bb) => {
            format!("{}_5BET{}({}bb)", ca.alias, ai_suffix, fmt_trimmed(*bb, 1))
        }
        ActionLabel::Bet(pot_frac) => {
            format!(
                "{}_BET{}({}pot)",
                ca.alias,
                ai_suffix,
                fmt_trimmed(*pot_frac, 2)
            )
        }
        ActionLabel::CBet(pot_frac) => {
            format!(
                "{}_CBET{}({}pot)",
                ca.alias,
                ai_suffix,
                fmt_trimmed(*pot_frac, 2)
            )
        }
        ActionLabel::Raise(multiplier) => {
            format!(
                "{}_RAISE{}({}x)",
                ca.alias,
                ai_suffix,
                fmt_trimmed(*multiplier, 1)
            )
        }
        ActionLabel::CallBB(bb_size) => {
            format!(
                "{}_CALL{}({}bb)",
                ca.alias,
                ai_suffix,
                fmt_trimmed(*bb_size, 1)
            )
        }
        ActionLabel::CallPot(pot_frac) => {
            format!(
                "{}_CALL{}({}pot)",
                ca.alias,
                ai_suffix,
                fmt_trimmed(*pot_frac, 2)
            )
        }
        ActionLabel::Check => "CHECK".to_string(),
        ActionLabel::Fold => format!("{}_FOLD", ca.alias),
        ActionLabel::BringIn(bb) => {
            format!("{}_BRINGIN({}bb)", ca.alias, fmt_trimmed(*bb, 1))
        }
    }
}
