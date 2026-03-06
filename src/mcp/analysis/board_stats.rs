use std::collections::HashMap;

use crate::stats;
use crate::types::{ActionType, Card, Hand, Street};

use crate::mcp::helpers::rank_order;

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
