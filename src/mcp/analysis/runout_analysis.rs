use std::collections::HashMap;

use crate::stats;
use crate::types::{Card, Hand, Street};

use crate::mcp::helpers::rank_order;

fn suit_texture(board: &[Card]) -> &'static str {
    if board.len() < 3 {
        return "unknown";
    }
    let s1 = board[0].suit;
    let s2 = board[1].suit;
    let s3 = board[2].suit;
    if s1 == s2 && s2 == s3 {
        "monotone"
    } else if s1 != s2 && s2 != s3 && s1 != s3 {
        "rainbow"
    } else {
        "two-tone"
    }
}

fn is_paired(board: &[Card]) -> bool {
    let mut seen = [0u8; 15];
    for c in board {
        let r = rank_order(c.rank) as usize;
        seen[r] += 1;
        if seen[r] > 1 {
            return true;
        }
    }
    false
}

fn is_connected(board: &[Card]) -> bool {
    if board.len() < 3 {
        return false;
    }
    let mut ranks: Vec<u8> = board.iter().map(|c| rank_order(c.rank)).collect();
    ranks.sort();
    ranks.dedup();
    // Check if any 3 consecutive ranks span <= 4
    if ranks.len() >= 3 {
        for w in ranks.windows(3) {
            if w[2] - w[0] <= 4 {
                return true;
            }
        }
    }
    false
}

fn has_flush(board: &[Card]) -> bool {
    let mut suit_counts = [0u8; 4];
    for c in board {
        let idx = match c.suit {
            crate::types::Suit::Clubs => 0,
            crate::types::Suit::Diamonds => 1,
            crate::types::Suit::Hearts => 2,
            crate::types::Suit::Spades => 3,
        };
        suit_counts[idx] += 1;
        if suit_counts[idx] >= 3 {
            return true;
        }
    }
    false
}

fn board_texture_label(board: &[Card]) -> String {
    if board.len() < 3 {
        return "unknown".to_string();
    }

    let mut labels = Vec::new();

    // Suit texture (of full board)
    labels.push(suit_texture(&board[..3]).to_string());

    if is_paired(board) {
        labels.push("paired".to_string());
    }
    if is_connected(&board[..3]) {
        labels.push("connected".to_string());
    }
    if board.len() >= 4 && has_flush(&board[..4]) {
        labels.push("4-flush".to_string());
    }
    if board.len() >= 5 && has_flush(board) {
        labels.push("5-flush".to_string());
    }

    // Highness
    let high_cards = board
        .iter()
        .take(3)
        .filter(|c| rank_order(c.rank) >= 10)
        .count();
    if high_cards >= 2 {
        labels.push("high".to_string());
    } else if high_cards == 0 {
        labels.push("low".to_string());
    }

    labels.join(", ")
}

pub fn get_runout_analysis(hands: &[Hand], hero: &str) -> serde_json::Value {
    // Classify hands by final board texture and compute win rate
    struct Bucket {
        hands: u64,
        wins: u64,
        profit_bb: f64,
    }

    let mut buckets: HashMap<String, Bucket> = HashMap::new();
    let mut total = 0u64;

    for hand in hands {
        let in_hand = hand
            .players
            .iter()
            .any(|p| p.name == hero && !p.is_sitting_out);
        if !in_hand {
            continue;
        }
        if hand.board.len() < 3 {
            continue;
        }

        // Hero must have seen the flop
        let hero_saw_flop = hand
            .actions
            .iter()
            .any(|a| a.player == hero && a.street == Street::Flop);
        if !hero_saw_flop {
            continue;
        }

        total += 1;

        let bb = stats::big_blind_size(hand);
        let profit = stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
        let profit_bb_val = if bb > 0.0 { profit / bb } else { 0.0 };
        let won = profit > 0.0;

        let label = board_texture_label(&hand.board);

        let b = buckets.entry(label).or_insert(Bucket {
            hands: 0,
            wins: 0,
            profit_bb: 0.0,
        });
        b.hands += 1;
        if won {
            b.wins += 1;
        }
        b.profit_bb += profit_bb_val;
    }

    let pct = |n: u64, d: u64| -> f64 {
        if d > 0 {
            n as f64 / d as f64 * 100.0
        } else {
            0.0
        }
    };

    let mut textures: Vec<serde_json::Value> = buckets
        .iter()
        .filter(|(_, b)| b.hands >= 2)
        .map(|(label, b)| {
            let winrate = b.profit_bb / b.hands as f64 * 100.0;
            serde_json::json!({
                "board_texture": label,
                "hands": b.hands,
                "win_pct": format!("{:.1}", pct(b.wins, b.hands)),
                "winrate_bb100": format!("{:.1}", winrate),
                "profit_bb": format!("{:.1}", b.profit_bb),
            })
        })
        .collect();

    // Sort by hand count descending
    textures.sort_by(|a, b| {
        let ha = a["hands"].as_u64().unwrap_or(0);
        let hb = b["hands"].as_u64().unwrap_or(0);
        hb.cmp(&ha)
    });

    serde_json::json!({
        "player": hero,
        "total_hands": total,
        "board_textures": textures,
    })
}
