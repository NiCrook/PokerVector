use std::collections::HashMap;

use crate::stats;
use crate::types::{ActionType, Hand, Street};

use crate::mcp::helpers::combo_label;

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
