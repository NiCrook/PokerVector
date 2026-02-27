use std::collections::HashMap;

use crate::types::*;
use super::calculate::calculate_stats;
use super::types::PlayerStats;

/// Distribution stats for a single metric across the player pool.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatDistribution {
    pub mean: f64,
    pub median: f64,
    pub p25: f64,
    pub p75: f64,
    pub min: f64,
    pub max: f64,
}

/// Aggregate pool statistics across all non-hero players.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolStats {
    pub total_players: u64,
    pub total_hands: u64,
    pub vpip: StatDistribution,
    pub pfr: StatDistribution,
    pub three_bet_pct: StatDistribution,
    pub fold_to_three_bet: StatDistribution,
    pub aggression_factor: StatDistribution,
    pub cbet_flop: StatDistribution,
    pub fold_to_cbet_flop: StatDistribution,
    pub steal_pct: StatDistribution,
    pub wwsf: StatDistribution,
    pub went_to_showdown_pct: StatDistribution,
    pub won_at_showdown_pct: StatDistribution,
    pub cold_call_pct: StatDistribution,
    pub check_raise_pct: StatDistribution,
    pub limp_pct: StatDistribution,
    pub flop_seen_pct: StatDistribution,
    pub aggression_frequency: StatDistribution,
}

fn compute_distribution(values: &mut Vec<f64>) -> StatDistribution {
    if values.is_empty() {
        return StatDistribution {
            mean: 0.0,
            median: 0.0,
            p25: 0.0,
            p75: 0.0,
            min: 0.0,
            max: 0.0,
        };
    }

    // Filter out infinities for AF
    values.retain(|v| v.is_finite());
    if values.is_empty() {
        return StatDistribution {
            mean: 0.0,
            median: 0.0,
            p25: 0.0,
            p75: 0.0,
            min: 0.0,
            max: 0.0,
        };
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    let sum: f64 = values.iter().sum();

    StatDistribution {
        mean: sum / n as f64,
        median: percentile(values, 50.0),
        p25: percentile(values, 25.0),
        p75: percentile(values, 75.0),
        min: values[0],
        max: values[n - 1],
    }
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (pct / 100.0) * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = idx - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

/// Calculate pool stats for all non-hero players in the given hands.
///
/// Each unique opponent with at least `min_hands` is profiled via `calculate_stats`,
/// then their stats are aggregated into distributions.
pub fn calculate_pool_stats(hands: &[Hand], hero: &str, min_hands: u64) -> PoolStats {
    // Group hands by non-hero player
    let mut player_hands: HashMap<String, Vec<Hand>> = HashMap::new();

    for hand in hands {
        let hero_in_hand = hand.players.iter().any(|p| p.name == hero && !p.is_sitting_out);
        if !hero_in_hand {
            continue;
        }
        for player in &hand.players {
            if player.name != hero && !player.is_sitting_out {
                player_hands
                    .entry(player.name.clone())
                    .or_default()
                    .push(hand.clone());
            }
        }
    }

    // Compute per-player stats
    let player_stats: Vec<PlayerStats> = player_hands
        .into_iter()
        .filter(|(_, h)| h.len() as u64 >= min_hands)
        .map(|(name, h)| calculate_stats(&h, &name))
        .collect();

    let total_players = player_stats.len() as u64;
    let total_hands: u64 = player_stats.iter().map(|s| s.hands_played).sum();

    // Extract per-stat vectors
    macro_rules! collect_stat {
        ($field:ident) => {{
            let mut v: Vec<f64> = player_stats.iter().map(|s| s.$field).collect();
            compute_distribution(&mut v)
        }};
    }

    PoolStats {
        total_players,
        total_hands,
        vpip: collect_stat!(vpip),
        pfr: collect_stat!(pfr),
        three_bet_pct: collect_stat!(three_bet_pct),
        fold_to_three_bet: collect_stat!(fold_to_three_bet),
        aggression_factor: collect_stat!(aggression_factor),
        cbet_flop: collect_stat!(cbet_flop),
        fold_to_cbet_flop: collect_stat!(fold_to_cbet_flop),
        steal_pct: collect_stat!(steal_pct),
        wwsf: collect_stat!(wwsf),
        went_to_showdown_pct: collect_stat!(went_to_showdown_pct),
        won_at_showdown_pct: collect_stat!(won_at_showdown_pct),
        cold_call_pct: collect_stat!(cold_call_pct),
        check_raise_pct: collect_stat!(check_raise_pct),
        limp_pct: collect_stat!(limp_pct),
        flop_seen_pct: collect_stat!(flop_seen_pct),
        aggression_frequency: collect_stat!(aggression_frequency),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_helpers::*;

    #[test]
    fn test_pool_stats_basic() {
        // Two villains, each with 1 hand
        let mut h1 = base_hand();
        h1.actions = vec![
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(0.02),
                    to: make_money(0.06),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(0.05),
                    all_in: false,
                },
                street: Street::Preflop,
            },
        ];

        let mut h2 = base_hand();
        h2.id = 2;
        // Replace Villain with Fish in this hand
        h2.players = vec![
            Player {
                name: "Hero".to_string(),
                seat: 1,
                stack: make_money(5.00),
                position: Some(Position::BTN),
                is_hero: true,
                is_sitting_out: false,
            },
            Player {
                name: "Fish".to_string(),
                seat: 2,
                stack: make_money(5.00),
                position: Some(Position::BB),
                is_hero: false,
                is_sitting_out: false,
            },
        ];
        h2.actions = vec![
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(0.02),
                    to: make_money(0.06),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
        ];

        let pool = calculate_pool_stats(&[h1, h2], "Hero", 1);
        assert_eq!(pool.total_players, 2);
        // Villain: VPIP 100 (called), Fish: VPIP 0 (folded)
        // mean VPIP = 50
        assert!((pool.vpip.mean - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_pool_stats_min_hands_filter() {
        let mut h1 = base_hand();
        h1.actions = vec![
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
        ];

        // Only 1 hand for Villain, min_hands=2 should exclude them
        let pool = calculate_pool_stats(&[h1], "Hero", 2);
        assert_eq!(pool.total_players, 0);
    }

    #[test]
    fn test_pool_stats_empty() {
        let pool = calculate_pool_stats(&[], "Hero", 1);
        assert_eq!(pool.total_players, 0);
        assert!((pool.vpip.mean - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_distribution_percentiles() {
        // 4 values: 10, 20, 30, 40
        let mut vals = vec![10.0, 20.0, 30.0, 40.0];
        let dist = super::compute_distribution(&mut vals);
        assert!((dist.min - 10.0).abs() < 0.01);
        assert!((dist.max - 40.0).abs() < 0.01);
        assert!((dist.mean - 25.0).abs() < 0.01);
        // median of [10,20,30,40] = interpolated at index 1.5 = 25
        assert!((dist.median - 25.0).abs() < 0.01);
        // p25 at index 0.75 = 10*0.25 + 20*0.75 = 17.5
        assert!((dist.p25 - 17.5).abs() < 0.01);
        // p75 at index 2.25 = 30*0.75 + 40*0.25 = 32.5
        assert!((dist.p75 - 32.5).abs() < 0.01);
    }
}
