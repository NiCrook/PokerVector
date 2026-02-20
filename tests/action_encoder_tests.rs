use pokervector::action_encoder::encode_action_sequence;
use pokervector::parsers::*;

fn load_fixture(name: &str) -> String {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {}", path))
}

fn parse_first_hand(fixture: &str, hero: &str) -> pokervector::types::Hand {
    let content = load_fixture(fixture);
    let results = parse_auto(&content, hero);
    results
        .into_iter()
        .next()
        .expect("no results")
        .expect("parse failed")
}

#[test]
fn test_encoding_deterministic() {
    let hand = parse_first_hand("cash_simple.txt", "PolarFox");
    let enc1 = encode_action_sequence(&hand, "PolarFox");
    let enc2 = encode_action_sequence(&hand, "PolarFox");
    assert_eq!(enc1, enc2, "Encoding should be deterministic");
}

#[test]
fn test_encoding_cash_simple_structure() {
    let hand = parse_first_hand("cash_simple.txt", "PolarFox");
    let encoded = encode_action_sequence(&hand, "PolarFox");

    // Should have PRE: line
    assert!(
        encoded.contains("PRE:"),
        "Cash hand should have preflop line, got:\n{}",
        encoded
    );
    // Should have RESULT: line
    assert!(
        encoded.contains("RESULT: HERO("),
        "Should have result line, got:\n{}",
        encoded
    );
    // Should not be SAT_OUT
    assert_ne!(encoded, "SAT_OUT");
}

#[test]
fn test_encoding_stud_format() {
    let hand = parse_first_hand("stud_hl.txt", "PolarFox");
    let encoded = encode_action_sequence(&hand, "PolarFox");

    // Should have stud street labels
    assert!(
        encoded.contains("3RD:"),
        "Stud hand should have 3RD: label, got:\n{}",
        encoded
    );
    // Should NOT have board cards (no brackets)
    assert!(
        !encoded.contains("["),
        "Stud hand should have no board cards, got:\n{}",
        encoded
    );
    // Should NOT have PRE: (stud has no preflop)
    assert!(
        !encoded.contains("PRE:"),
        "Stud hand should not have PRE: label, got:\n{}",
        encoded
    );
}

#[test]
fn test_encoding_bomb_pot() {
    let hand = parse_first_hand("bomb_pot.txt", "PolarFox");
    let encoded = encode_action_sequence(&hand, "PolarFox");

    assert!(
        encoded.starts_with("BOMB_POT"),
        "Bomb pot should start with BOMB_POT, got:\n{}",
        encoded
    );
    assert!(
        !encoded.contains("PRE:"),
        "Bomb pot should have no preflop, got:\n{}",
        encoded
    );
}

#[test]
fn test_encoding_uses_hero_alias() {
    let hand = parse_first_hand("cash_simple.txt", "PolarFox");
    let encoded = encode_action_sequence(&hand, "PolarFox");

    // PolarFox should be anonymized as HERO
    assert!(
        !encoded.contains("PolarFox"),
        "Hero name should be anonymized, got:\n{}",
        encoded
    );
    assert!(
        encoded.contains("HERO"),
        "Should use HERO alias, got:\n{}",
        encoded
    );
}

#[test]
fn test_encoding_sitting_out() {
    let hand = parse_first_hand("sitting_out.txt", "PolarFox");
    let encoded = encode_action_sequence(&hand, "PolarFox");

    // If hero sat out, should be SAT_OUT
    if hand.result.hero_result == pokervector::types::HeroResult::SatOut {
        assert_eq!(encoded, "SAT_OUT");
    }
}

#[test]
fn test_encoding_all_fixtures_no_panic() {
    let fixtures = [
        "cash_simple.txt",
        "cash_showdown.txt",
        "cash_ante.txt",
        "tournament_basic.txt",
        "split_pot.txt",
        "side_pots.txt",
        "multiword_name.txt",
        "sitting_out.txt",
        "hero_allin.txt",
        "omaha_hl.txt",
        "five_card_omaha.txt",
        "stud_hl.txt",
        "bomb_pot.txt",
        "omaha_plo.txt",
    ];

    for fixture in &fixtures {
        let content = load_fixture(fixture);
        let results = parse_auto(&content, "PolarFox");
        for result in results {
            if let Ok(hand) = result {
                // Should not panic
                let encoded = encode_action_sequence(&hand, "PolarFox");
                assert!(
                    !encoded.is_empty(),
                    "Encoding should not be empty for {}",
                    fixture
                );
            }
        }
    }
}

#[test]
fn test_encoding_omaha_has_streets() {
    let hand = parse_first_hand("omaha_hl.txt", "PolarFox");
    let encoded = encode_action_sequence(&hand, "PolarFox");

    // Omaha should use standard Hold'em street labels
    assert!(
        encoded.contains("PRE:") || encoded.contains("FLOP"),
        "Omaha should use standard street labels, got:\n{}",
        encoded
    );
}
