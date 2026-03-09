use pokervector::parsers::*;
use pokervector::types::*;

fn load_fixture(name: &str) -> String {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {}", path))
}

#[test]
fn test_parse_card_utility() {
    let card = parse_card("Ah").unwrap();
    assert_eq!(card.rank, Rank::Ace);
    assert_eq!(card.suit, Suit::Hearts);

    let card = parse_card("Td").unwrap();
    assert_eq!(card.rank, Rank::Ten);
    assert_eq!(card.suit, Suit::Diamonds);

    assert!(parse_card("-").is_err());
    assert!(parse_card("X").is_err());
}

#[test]
fn test_parse_money_utility() {
    let m = parse_money("$0.05").unwrap();
    assert_eq!(m.currency, Currency::USD);
    assert!((m.amount - 0.05).abs() < 0.001);

    let m = parse_money("5000.00").unwrap();
    assert_eq!(m.currency, Currency::Chips);
    assert!((m.amount - 5000.0).abs() < 0.001);
}

#[test]
fn test_parse_cards_utility() {
    let cards = parse_cards("[Ah Kd]");
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].to_string(), "Ah");
    assert_eq!(cards[1].to_string(), "Kd");
}

#[test]
fn test_split_hands() {
    let content = "Hand #1\nline1\nline2\n\nHand #2\nline3\n\n";
    let hands = split_hands(content);
    assert_eq!(hands.len(), 2);
    assert!(hands[0].starts_with("Hand #1"));
    assert!(hands[1].starts_with("Hand #2"));
}

#[test]
fn test_calculate_position_9max() {
    let seats = vec![1, 2, 3, 4, 7, 8, 9];
    // Button at seat 9
    assert_eq!(calculate_position(9, 9, &seats), Some(Position::BTN));
    assert_eq!(calculate_position(1, 9, &seats), Some(Position::SB));
    assert_eq!(calculate_position(2, 9, &seats), Some(Position::BB));
    assert_eq!(calculate_position(3, 9, &seats), Some(Position::UTG));
    assert_eq!(calculate_position(4, 9, &seats), Some(Position::MP1));
    assert_eq!(calculate_position(7, 9, &seats), Some(Position::HJ));
    assert_eq!(calculate_position(8, 9, &seats), Some(Position::CO));
}

#[test]
fn test_calculate_position_headsup() {
    let seats = vec![2, 4];
    assert_eq!(calculate_position(2, 2, &seats), Some(Position::BTN));
    assert_eq!(calculate_position(4, 2, &seats), Some(Position::BB));
}

#[test]
fn test_detect_acr_cash() {
    assert!(acr::AcrParser::detect(
        "Hand #2651598865 - Holdem (No Limit) - $0.01/$0.02 - 2026/01/22 UTC"
    ));
}

#[test]
fn test_detect_acr_tournament() {
    assert!(acr::AcrParser::detect(
        "Game Hand #2653060401 - Tournament #34375286"
    ));
}

#[test]
fn test_detect_unknown() {
    assert!(!acr::AcrParser::detect("PokerStars Hand #123"));
}

#[test]
fn test_auto_detect_routes() {
    let content = load_fixture("cash_simple.txt");
    let results = parse_auto(&content, "TestHero");
    assert_eq!(results.len(), 1);
    assert!(results[0].is_ok());
}

// --- Full hand parse tests ---

#[test]
fn test_cash_simple() {
    let content = load_fixture("cash_simple.txt");
    let results = parse_auto(&content, "TestHero");
    assert_eq!(results.len(), 1);
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.id, 2651598865);
    assert_eq!(hand.site, Site::ACR);
    assert_eq!(hand.table_name, "McCook");
    assert_eq!(hand.table_size, 9);
    assert_eq!(hand.button_seat, 9);
    assert_eq!(hand.players.len(), 7);

    // Hero identification
    assert_eq!(hand.hero, Some("TestHero".to_string()));
    assert_eq!(hand.hero_position, Some(Position::BB));
    assert_eq!(hand.hero_cards.len(), 2);
    assert_eq!(hand.hero_cards[0].to_string(), "Qd");
    assert_eq!(hand.hero_cards[1].to_string(), "Jc");

    // Game type
    match &hand.game_type {
        GameType::Cash {
            small_blind,
            big_blind,
            ante,
        } => {
            assert!((small_blind.amount - 0.01).abs() < 0.001);
            assert!((big_blind.amount - 0.02).abs() < 0.001);
            assert!(ante.is_none());
        }
        _ => panic!("expected cash game"),
    }

    // Result: hero won
    assert_eq!(hand.result.hero_result, HeroResult::Won);
    assert_eq!(hand.result.winners.len(), 1);
    assert_eq!(hand.result.winners[0].player, "TestHero");
    assert!((hand.result.winners[0].amount.amount - 0.06).abs() < 0.001);
}

#[test]
fn test_cash_showdown() {
    let content = load_fixture("cash_showdown.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.id, 2651599519);
    assert_eq!(hand.hero_position, Some(Position::SB));
    assert_eq!(hand.board.len(), 5);
    assert_eq!(hand.board[0].to_string(), "6h");
    assert_eq!(hand.board[4].to_string(), "8d");

    // Hero lost at showdown
    assert_eq!(hand.result.hero_result, HeroResult::Lost);
    // Winner should be Freddeyz
    assert!(hand.result.winners.iter().any(|w| w.player == "Freddeyz"));

    // Pot and rake
    assert!(hand.pot.is_some());
    assert!(hand.rake.is_some());
}

#[test]
fn test_cash_ante() {
    let content = load_fixture("cash_ante.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.id, 2662089748);
    assert_eq!(hand.table_name, "Colwich");
    assert_eq!(hand.table_size, 6);

    match &hand.game_type {
        GameType::Cash { ante, .. } => {
            assert!(ante.is_some());
            assert!((ante.unwrap().amount - 0.01).abs() < 0.001);
        }
        _ => panic!("expected cash game"),
    }

    // Check ante actions exist
    let ante_actions: Vec<_> = hand
        .actions
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::PostAnte { .. }))
        .collect();
    assert_eq!(ante_actions.len(), 2);
}

#[test]
fn test_tournament_basic() {
    let content = load_fixture("tournament_basic.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.id, 2653060401);
    assert_eq!(hand.table_name, "36");
    assert_eq!(hand.table_size, 8);
    assert_eq!(hand.players.len(), 8);

    match &hand.game_type {
        GameType::Tournament {
            tournament_id,
            level,
            small_blind,
            big_blind,
            ..
        } => {
            assert_eq!(*tournament_id, 34375286);
            assert_eq!(*level, 17);
            assert!((small_blind.amount - 2500.0).abs() < 0.01);
            assert!((big_blind.amount - 5000.0).abs() < 0.01);
            assert_eq!(small_blind.currency, Currency::Chips);
        }
        _ => panic!("expected tournament"),
    }

    // Antes exist
    let ante_actions: Vec<_> = hand
        .actions
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::PostAnte { .. }))
        .collect();
    assert_eq!(ante_actions.len(), 8);

    // Hero folded
    assert_eq!(hand.result.hero_result, HeroResult::Folded);
}

#[test]
fn test_split_pot() {
    let content = load_fixture("split_pot.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    // Two winners splitting the pot
    assert!(hand.result.winners.len() >= 2);
    let winner_names: Vec<&str> = hand
        .result
        .winners
        .iter()
        .map(|w| w.player.as_str())
        .collect();
    assert!(winner_names.contains(&"PokerBossBabe"));
    assert!(winner_names.contains(&"ksedoks"));

    // Hero lost
    assert_eq!(hand.result.hero_result, HeroResult::Lost);
}

#[test]
fn test_side_pots() {
    let content = load_fixture("side_pots.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.id, 2651598942);

    // NineABS collected from main pot + 2 side pots
    let nine_abs_wins: Vec<_> = hand
        .result
        .winners
        .iter()
        .filter(|w| w.player == "NineABS")
        .collect();
    assert_eq!(nine_abs_wins.len(), 3);

    // Verify pot names
    let pot_names: Vec<&str> = nine_abs_wins.iter().map(|w| w.pot.as_str()).collect();
    assert!(pot_names.contains(&"main pot"));
    assert!(pot_names.contains(&"side pot-1"));
    assert!(pot_names.contains(&"side pot-2"));
}

#[test]
fn test_multiword_name() {
    let content = load_fixture("multiword_name.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    // "Lost It" should be a valid player
    let lost_it = hand.players.iter().find(|p| p.name == "Lost It");
    assert!(lost_it.is_some(), "Player 'Lost It' not found");
    assert_eq!(lost_it.unwrap().seat, 7);

    // "Lost It" should have posted ante and small blind
    let lost_it_actions: Vec<_> = hand
        .actions
        .iter()
        .filter(|a| a.player == "Lost It")
        .collect();
    assert!(
        !lost_it_actions.is_empty(),
        "No actions found for 'Lost It'"
    );

    // Check that Lost It posted ante
    assert!(lost_it_actions
        .iter()
        .any(|a| matches!(a.action_type, ActionType::PostAnte { .. })));
    // Check that Lost It posted small blind
    assert!(lost_it_actions
        .iter()
        .any(|a| matches!(a.action_type, ActionType::PostSmallBlind { .. })));
    // Check that Lost It folded
    assert!(lost_it_actions
        .iter()
        .any(|a| a.action_type == ActionType::Fold));
}

#[test]
fn test_sitting_out() {
    let content = load_fixture("sitting_out.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    // SIA73 is sitting out
    let sia = hand.players.iter().find(|p| p.name == "SIA73").unwrap();
    assert!(sia.is_sitting_out);
    assert!(sia.position.is_none());

    // Other players should have positions
    let polar = hand.players.iter().find(|p| p.name == "TestHero").unwrap();
    assert!(!polar.is_sitting_out);
    assert!(polar.position.is_some());
}

#[test]
fn test_hero_allin() {
    let content = load_fixture("hero_allin.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.id, 2651601073);
    assert_eq!(hand.hero_cards.len(), 2);
    assert_eq!(hand.hero_cards[0].to_string(), "8s");
    assert_eq!(hand.hero_cards[1].to_string(), "8h");

    // Hero went all-in and won
    assert_eq!(hand.result.hero_result, HeroResult::Won);
    assert!((hand.result.winners[0].amount.amount - 3.87).abs() < 0.01);

    // Board should have 5 cards
    assert_eq!(hand.board.len(), 5);

    // Check all-in action exists
    let allin_action = hand.actions.iter().find(|a| {
        a.player == "TestHero" && matches!(a.action_type, ActionType::Raise { all_in: true, .. })
    });
    assert!(allin_action.is_some());
}

// --- Variant-specific tests ---

#[test]
fn test_omaha_hl() {
    let content = load_fixture("omaha_hl.txt");
    let results = parse_auto(&content, "TestHero");
    assert_eq!(results.len(), 1);
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.variant, PokerVariant::Omaha);
    assert_eq!(hand.betting_limit, BettingLimit::FixedLimit);
    assert!(hand.is_hi_lo);
    assert!(!hand.is_bomb_pot);

    // Omaha deals 4 hole cards
    assert_eq!(hand.hero_cards.len(), 4);

    // Board should have 5 cards
    assert_eq!(hand.board.len(), 5);

    // H/L split: both Lost It and bootanuts collected from main pot
    assert!(hand.result.winners.len() >= 2);
    let winner_names: Vec<&str> = hand
        .result
        .winners
        .iter()
        .map(|w| w.player.as_str())
        .collect();
    assert!(
        winner_names.contains(&"Lost It"),
        "Lost It should be a winner"
    );
    assert!(
        winner_names.contains(&"bootanuts"),
        "bootanuts should be a winner"
    );

    // No stud cards
    assert!(hand.stud_cards.is_none());
}

#[test]
fn test_omaha_plo() {
    let content = load_fixture("omaha_plo.txt");
    let results = parse_auto(&content, "TestHero");
    assert_eq!(results.len(), 1);
    let hand = results[0].as_ref().unwrap();

    // Standard PLO: Pot Limit Omaha (not Hi/Lo)
    assert_eq!(hand.variant, PokerVariant::Omaha);
    assert_eq!(hand.betting_limit, BettingLimit::PotLimit);
    assert!(!hand.is_hi_lo);
    assert!(!hand.is_bomb_pot);

    // 4 hole cards
    assert_eq!(hand.hero_cards.len(), 4);

    // Board should have 5 cards (went to showdown)
    assert_eq!(hand.board.len(), 5);

    // Hero (TestHero) won with a straight
    assert_eq!(hand.result.hero_result, HeroResult::Won);
    let winner_names: Vec<&str> = hand
        .result
        .winners
        .iter()
        .map(|w| w.player.as_str())
        .collect();
    assert!(winner_names.contains(&"TestHero"));

    // No stud cards
    assert!(hand.stud_cards.is_none());
}

#[test]
fn test_five_card_omaha() {
    let content = load_fixture("five_card_omaha.txt");
    let results = parse_auto(&content, "TestHero");
    assert_eq!(results.len(), 1);
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.variant, PokerVariant::FiveCardOmaha);
    assert_eq!(hand.betting_limit, BettingLimit::PotLimit);
    assert!(!hand.is_hi_lo);

    // 5-Card Omaha deals 5 hole cards
    assert_eq!(hand.hero_cards.len(), 5);

    // Board should have 5 cards (went to showdown)
    assert_eq!(hand.board.len(), 5);

    // Hero won
    assert_eq!(hand.result.hero_result, HeroResult::Won);
}

#[test]
fn test_stud_hl() {
    let content = load_fixture("stud_hl.txt");
    let results = parse_auto(&content, "TestHero");
    assert_eq!(results.len(), 1);
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.variant, PokerVariant::SevenCardStud);
    assert_eq!(hand.betting_limit, BettingLimit::FixedLimit);
    assert!(hand.is_hi_lo);

    // No button in stud
    assert_eq!(hand.button_seat, 0);

    // No board in stud
    assert!(hand.board.is_empty());

    // Hero should have cards (3 on 3rd street: 2 hidden + 1 up, then 1 more on 4th = 4 total)
    assert!(
        hand.hero_cards.len() >= 3,
        "Hero should have at least 3 cards, got {}",
        hand.hero_cards.len()
    );

    // Stud cards should be populated
    assert!(hand.stud_cards.is_some());
    let stud_cards = hand.stud_cards.as_ref().unwrap();
    assert!(!stud_cards.is_empty());

    // Verify brings_in action exists
    let brings_in = hand
        .actions
        .iter()
        .find(|a| matches!(a.action_type, ActionType::BringsIn { .. }));
    assert!(brings_in.is_some(), "Should have a brings_in action");

    // No positions assigned in stud
    for player in &hand.players {
        assert!(
            player.position.is_none(),
            "Stud players should not have positions"
        );
    }

    // Hero folded on 4th street
    assert_eq!(hand.result.hero_result, HeroResult::Folded);
}

#[test]
fn test_bomb_pot() {
    let content = load_fixture("bomb_pot.txt");
    let results = parse_auto(&content, "TestHero");
    assert_eq!(results.len(), 1);
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.variant, PokerVariant::Holdem);
    assert_eq!(hand.betting_limit, BettingLimit::NoLimit);
    assert!(!hand.is_hi_lo);
    assert!(hand.is_bomb_pot, "Should be flagged as bomb pot");

    // Still has 2 hole cards
    assert_eq!(hand.hero_cards.len(), 2);

    // Hero won
    assert_eq!(hand.result.hero_result, HeroResult::Won);
}

#[test]
fn test_existing_hands_have_holdem_defaults() {
    // Verify existing holdem hands get correct defaults
    let content = load_fixture("cash_simple.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();

    assert_eq!(hand.variant, PokerVariant::Holdem);
    assert_eq!(hand.betting_limit, BettingLimit::NoLimit);
    assert!(!hand.is_hi_lo);
    assert!(!hand.is_bomb_pot);
    assert!(hand.stud_cards.is_none());
}

#[test]
fn test_compact_output_cash_simple() {
    let content = load_fixture("cash_simple.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();
    let compact = hand.to_compact();

    // Core fields
    assert_eq!(compact["id"], 2651598865u64);
    assert_eq!(compact["variant"], "No Limit Hold'em");
    assert_eq!(compact["stakes"], "0.01/0.02");
    assert_eq!(compact["hero"], "TestHero");
    assert_eq!(compact["hero_position"], "BB");
    assert_eq!(compact["hero_cards"], serde_json::json!(["Qd", "Jc"]));
    assert_eq!(compact["table"], "McCook 9-max");

    // No raw_text, site, button_seat, currency
    assert!(compact.get("raw_text").is_none());
    assert!(compact.get("site").is_none());
    assert!(compact.get("button_seat").is_none());

    // Players as compact arrays
    let players = compact["players"].as_array().unwrap();
    assert_eq!(players.len(), 7);
    assert_eq!(players[0][0], "Ddrupp");
    assert_eq!(players[0][1], "SB");

    // Actions grouped by street, no blind posts
    let preflop = compact["preflop"].as_array().unwrap();
    assert!(!preflop.is_empty());
    // First action should NOT be a blind post
    let first = preflop[0].as_str().unwrap();
    assert!(
        !first.contains("posts"),
        "Blind posts should be omitted: {}",
        first
    );
    // Should contain hero's raise
    assert!(preflop
        .iter()
        .any(|a| a.as_str().unwrap().contains("raises to")));

    // No board for preflop-only hand
    assert!(compact.get("board").is_none());

    // Result
    assert_eq!(compact["result"], "Won 0.06");

    // No hi_lo or bomb_pot keys (false values omitted)
    assert!(compact.get("hi_lo").is_none());
    assert!(compact.get("bomb_pot").is_none());

    // Verify compactness — should be much smaller than full JSON
    let compact_str = serde_json::to_string(&compact).unwrap();
    let full_str = serde_json::to_string(hand).unwrap();
    assert!(
        compact_str.len() < full_str.len() / 2,
        "Compact ({}) should be less than half of full ({})",
        compact_str.len(),
        full_str.len()
    );
}

#[test]
fn test_compact_output_showdown() {
    let content = load_fixture("cash_showdown.txt");
    let results = parse_auto(&content, "TestHero");
    let hand = results[0].as_ref().unwrap();
    let compact = hand.to_compact();

    // Should have multiple streets
    assert!(compact.get("preflop").is_some());
    assert!(
        compact.get("flop").is_some()
            || compact.get("turn").is_some()
            || compact.get("river").is_some(),
        "Multistreet hand should have postflop actions"
    );

    // Board should be present
    assert!(compact.get("board").is_some());

    // Showdown actions should be present if players showed
    let has_shows = hand
        .actions
        .iter()
        .any(|a| matches!(a.action_type, ActionType::Shows { .. }));
    if has_shows {
        assert!(compact.get("showdown").is_some());
    }
}

#[test]
fn test_compact_action_call_no_amount() {
    let action = Action {
        player: "TestPlayer".to_string(),
        action_type: ActionType::Call {
            amount: Money {
                amount: 0.50,
                currency: Currency::USD,
            },
            all_in: false,
        },
        street: Street::Preflop,
    };
    let result = Hand::compact_action(&action).unwrap();
    assert_eq!(result, "TestPlayer calls");
    assert!(!result.contains("0.50"), "Call amount should be omitted");
}

#[test]
fn test_compact_action_call_allin() {
    let action = Action {
        player: "TestPlayer".to_string(),
        action_type: ActionType::Call {
            amount: Money {
                amount: 10.00,
                currency: Currency::USD,
            },
            all_in: true,
        },
        street: Street::Preflop,
    };
    let result = Hand::compact_action(&action).unwrap();
    assert_eq!(result, "TestPlayer calls (all-in)");
}

#[test]
fn test_compact_action_blinds_omitted() {
    let sb = Action {
        player: "Test".to_string(),
        action_type: ActionType::PostSmallBlind {
            amount: Money {
                amount: 0.01,
                currency: Currency::USD,
            },
            all_in: false,
        },
        street: Street::Preflop,
    };
    assert!(Hand::compact_action(&sb).is_none());

    let bb = Action {
        player: "Test".to_string(),
        action_type: ActionType::PostBigBlind {
            amount: Money {
                amount: 0.02,
                currency: Currency::USD,
            },
            all_in: false,
        },
        street: Street::Preflop,
    };
    assert!(Hand::compact_action(&bb).is_none());
}
