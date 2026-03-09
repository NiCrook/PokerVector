use std::collections::HashMap;

use crate::types::{ActionType, Card, Hand, Rank, Street};

use crate::mcp::helpers::rank_order;

fn rank_label(rank: Rank) -> &'static str {
    match rank {
        Rank::Two => "2",
        Rank::Three => "3",
        Rank::Four => "4",
        Rank::Five => "5",
        Rank::Six => "6",
        Rank::Seven => "7",
        Rank::Eight => "8",
        Rank::Nine => "9",
        Rank::Ten => "T",
        Rank::Jack => "J",
        Rank::Queen => "Q",
        Rank::King => "K",
        Rank::Ace => "A",
    }
}

fn suit_label(suit: crate::types::Suit) -> &'static str {
    match suit {
        crate::types::Suit::Clubs => "c",
        crate::types::Suit::Diamonds => "d",
        crate::types::Suit::Hearts => "h",
        crate::types::Suit::Spades => "s",
    }
}

/// Classify a turn/river card relative to the flop
fn classify_card(card: &Card, flop: &[Card]) -> Vec<&'static str> {
    let mut tags = Vec::new();

    let r = rank_order(card.rank);

    // Overcard to flop?
    let max_flop_rank = flop.iter().map(|c| rank_order(c.rank)).max().unwrap_or(0);
    if r > max_flop_rank {
        tags.push("overcard");
    }

    // Pairs the board?
    if flop.iter().any(|c| c.rank == card.rank) {
        tags.push("pairs_board");
    }

    // Completes flush draw? (3rd of a suit on flop)
    let card_suit = card.suit;
    let suit_count_on_flop = flop.iter().filter(|c| c.suit == card_suit).count();
    if suit_count_on_flop >= 2 {
        tags.push("flush_completing");
    }

    // High card (T+)?
    if r >= 10 {
        tags.push("broadway");
    }

    // Low card (2-6)?
    if r <= 6 {
        tags.push("low");
    }

    if tags.is_empty() {
        tags.push("neutral");
    }

    tags
}

pub fn get_runout_frequencies_analysis(
    hands: &[Hand],
    hero: &str,
    street: &str,
) -> serde_json::Value {
    // Filter hands where hero c-bet flop and got called (the default interesting scenario)
    // OR just any hand that reached the target street

    let target_street = match street {
        "turn" => Street::Turn,
        "river" => Street::River,
        _ => Street::Turn,
    };

    let card_index = match target_street {
        Street::Turn => 3,  // board[3] is the turn card
        Street::River => 4, // board[4] is the river card
        _ => 3,
    };

    let min_board_len = card_index + 1;

    // Count rank frequencies, card type frequencies
    let mut rank_counts: HashMap<&str, u64> = HashMap::new();
    let mut suit_counts: HashMap<&str, u64> = HashMap::new();
    let mut type_counts: HashMap<&'static str, u64> = HashMap::new();
    let mut total = 0u64;
    let mut cbet_called_total = 0u64;

    // Also track specifically for "hero c-bets flop and gets called" subset
    let mut cbet_rank_counts: HashMap<&str, u64> = HashMap::new();
    let mut cbet_type_counts: HashMap<&'static str, u64> = HashMap::new();

    for hand in hands {
        let in_hand = hand
            .players
            .iter()
            .any(|p| p.name == hero && !p.is_sitting_out);
        if !in_hand {
            continue;
        }
        if hand.board.len() < min_board_len {
            continue;
        }

        // Hand must have reached the target street
        let reached_street = hand.actions.iter().any(|a| a.street == target_street);
        if !reached_street {
            continue;
        }

        let card = &hand.board[card_index];
        let flop = &hand.board[..3];

        total += 1;

        // Count rank
        let rl = rank_label(card.rank);
        *rank_counts.entry(rl).or_default() += 1;

        // Count suit
        let sl = suit_label(card.suit);
        *suit_counts.entry(sl).or_default() += 1;

        // Classify card type
        for tag in classify_card(card, flop) {
            *type_counts.entry(tag).or_default() += 1;
        }

        // Check if hero c-bet flop and got called
        let hero_cbet = hand.actions.iter().any(|a| {
            a.player == hero
                && a.street == Street::Flop
                && matches!(&a.action_type, ActionType::Bet { .. })
        });
        let hero_was_pfr = hand.actions.iter().any(|a| {
            a.player == hero
                && a.street == Street::Preflop
                && matches!(&a.action_type, ActionType::Raise { .. })
        });
        let villain_called_flop = hand.actions.iter().any(|a| {
            a.player != hero
                && a.street == Street::Flop
                && matches!(&a.action_type, ActionType::Call { .. })
        });

        if hero_was_pfr && hero_cbet && villain_called_flop {
            cbet_called_total += 1;
            *cbet_rank_counts.entry(rl).or_default() += 1;
            for tag in classify_card(card, flop) {
                *cbet_type_counts.entry(tag).or_default() += 1;
            }
        }
    }

    let pct = |n: u64, d: u64| -> f64 {
        if d > 0 {
            n as f64 / d as f64 * 100.0
        } else {
            0.0
        }
    };

    // Build rank distribution sorted by frequency
    let mut rank_dist: Vec<serde_json::Value> = rank_counts
        .iter()
        .map(|(rank, count)| {
            serde_json::json!({
                "rank": rank,
                "count": count,
                "pct": format!("{:.1}", pct(*count, total)),
            })
        })
        .collect();
    rank_dist.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });

    // Build suit distribution
    let mut suit_dist: Vec<serde_json::Value> = suit_counts
        .iter()
        .map(|(suit, count)| {
            serde_json::json!({
                "suit": suit,
                "count": count,
                "pct": format!("{:.1}", pct(*count, total)),
            })
        })
        .collect();
    suit_dist.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });

    // Build type distribution (overcard, pairs_board, flush_completing, etc.)
    let mut type_dist: Vec<serde_json::Value> = type_counts
        .iter()
        .map(|(typ, count)| {
            serde_json::json!({
                "type": typ,
                "count": count,
                "pct": format!("{:.1}", pct(*count, total)),
            })
        })
        .collect();
    type_dist.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });

    // Build c-bet-called subset
    let cbet_section = if cbet_called_total > 0 {
        let mut cbet_rank_dist: Vec<serde_json::Value> = cbet_rank_counts
            .iter()
            .map(|(rank, count)| {
                serde_json::json!({
                    "rank": rank,
                    "count": count,
                    "pct": format!("{:.1}", pct(*count, cbet_called_total)),
                })
            })
            .collect();
        cbet_rank_dist.sort_by(|a, b| {
            b["count"]
                .as_u64()
                .unwrap_or(0)
                .cmp(&a["count"].as_u64().unwrap_or(0))
        });

        let mut cbet_type_dist: Vec<serde_json::Value> = cbet_type_counts
            .iter()
            .map(|(typ, count)| {
                serde_json::json!({
                    "type": typ,
                    "count": count,
                    "pct": format!("{:.1}", pct(*count, cbet_called_total)),
                })
            })
            .collect();
        cbet_type_dist.sort_by(|a, b| {
            b["count"]
                .as_u64()
                .unwrap_or(0)
                .cmp(&a["count"].as_u64().unwrap_or(0))
        });

        serde_json::json!({
            "total_hands": cbet_called_total,
            "rank_distribution": cbet_rank_dist,
            "card_types": cbet_type_dist,
        })
    } else {
        serde_json::json!({
            "total_hands": 0,
            "message": "No hands where hero c-bet flop and got called",
        })
    };

    serde_json::json!({
        "street": street,
        "total_hands": total,
        "rank_distribution": rank_dist,
        "suit_distribution": suit_dist,
        "card_types": type_dist,
        "after_hero_cbet_called": cbet_section,
    })
}
