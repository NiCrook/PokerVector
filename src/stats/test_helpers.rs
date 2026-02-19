use crate::types::*;

pub fn make_money(amount: f64) -> Money {
    Money { amount, currency: Currency::USD }
}

pub fn make_card(r: char, s: char) -> Card {
    Card {
        rank: Rank::from_char(r).unwrap(),
        suit: Suit::from_char(s).unwrap(),
    }
}

pub fn base_hand() -> Hand {
    Hand {
        id: 1,
        site: Site::ACR,
        variant: PokerVariant::Holdem,
        betting_limit: BettingLimit::NoLimit,
        is_hi_lo: false,
        is_bomb_pot: false,
        game_type: GameType::Cash {
            small_blind: make_money(0.01),
            big_blind: make_money(0.02),
            ante: None,
        },
        timestamp: "2024-01-01".to_string(),
        table_name: "Test".to_string(),
        table_size: 6,
        button_seat: 1,
        players: vec![
            Player { seat: 1, name: "Hero".to_string(), stack: make_money(2.00), position: Some(Position::BTN), is_hero: true, is_sitting_out: false },
            Player { seat: 2, name: "Villain".to_string(), stack: make_money(2.00), position: Some(Position::SB), is_hero: false, is_sitting_out: false },
            Player { seat: 3, name: "Fish".to_string(), stack: make_money(2.00), position: Some(Position::BB), is_hero: false, is_sitting_out: false },
        ],
        hero: Some("Hero".to_string()),
        hero_position: Some(Position::BTN),
        hero_cards: vec![make_card('A', 's'), make_card('K', 's')],
        actions: vec![],
        board: vec![],
        pot: Some(make_money(0.05)),
        rake: None,
        result: HandResult {
            winners: vec![],
            hero_result: HeroResult::Folded,
        },
        raw_text: String::new(),
        stud_cards: None,
    }
}

/// Helper: 6-max hand with positions for IP/OOP testing.
/// Hero=BTN, Villain=SB, Fish=BB, CO_Player=CO, HJ_Player=HJ, LJ_Player=LJ
pub fn sixmax_hand() -> Hand {
    Hand {
        id: 1,
        site: Site::ACR,
        variant: PokerVariant::Holdem,
        betting_limit: BettingLimit::NoLimit,
        is_hi_lo: false,
        is_bomb_pot: false,
        game_type: GameType::Cash {
            small_blind: make_money(0.50),
            big_blind: make_money(1.00),
            ante: None,
        },
        timestamp: "2024-01-01".to_string(),
        table_name: "Test6max".to_string(),
        table_size: 6,
        button_seat: 1,
        players: vec![
            Player { seat: 1, name: "Hero".to_string(), stack: make_money(100.0), position: Some(Position::BTN), is_hero: true, is_sitting_out: false },
            Player { seat: 2, name: "Villain".to_string(), stack: make_money(100.0), position: Some(Position::SB), is_hero: false, is_sitting_out: false },
            Player { seat: 3, name: "Fish".to_string(), stack: make_money(100.0), position: Some(Position::BB), is_hero: false, is_sitting_out: false },
            Player { seat: 4, name: "CO_Player".to_string(), stack: make_money(100.0), position: Some(Position::CO), is_hero: false, is_sitting_out: false },
            Player { seat: 5, name: "HJ_Player".to_string(), stack: make_money(100.0), position: Some(Position::HJ), is_hero: false, is_sitting_out: false },
            Player { seat: 6, name: "LJ_Player".to_string(), stack: make_money(100.0), position: Some(Position::LJ), is_hero: false, is_sitting_out: false },
        ],
        hero: Some("Hero".to_string()),
        hero_position: Some(Position::BTN),
        hero_cards: vec![make_card('A', 's'), make_card('K', 's')],
        actions: vec![],
        board: vec![],
        pot: Some(make_money(1.50)),
        rake: None,
        result: HandResult { winners: vec![], hero_result: HeroResult::Folded },
        raw_text: String::new(),
        stud_cards: None,
    }
}
