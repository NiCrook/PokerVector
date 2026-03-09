use super::*;

fn make_test_hand(id: u64) -> Hand {
    Hand {
        id,
        site: Site::ACR,
        variant: PokerVariant::Holdem,
        betting_limit: BettingLimit::NoLimit,
        is_hi_lo: false,
        is_bomb_pot: false,
        game_type: GameType::Cash {
            small_blind: Money {
                amount: 0.01,
                currency: Currency::USD,
            },
            big_blind: Money {
                amount: 0.02,
                currency: Currency::USD,
            },
            ante: None,
        },
        timestamp: "2024-01-15 12:00:00".to_string(),
        table_name: "TestTable".to_string(),
        table_size: 6,
        button_seat: 1,
        players: vec![
            Player {
                seat: 1,
                name: "Hero".to_string(),
                stack: Money {
                    amount: 2.00,
                    currency: Currency::USD,
                },
                position: Some(Position::BTN),
                is_hero: true,
                is_sitting_out: false,
            },
            Player {
                seat: 2,
                name: "Villain".to_string(),
                stack: Money {
                    amount: 2.00,
                    currency: Currency::USD,
                },
                position: Some(Position::SB),
                is_hero: false,
                is_sitting_out: false,
            },
        ],
        hero: Some("Hero".to_string()),
        hero_position: Some(Position::BTN),
        hero_cards: vec![
            Card {
                rank: Rank::Ace,
                suit: Suit::Spades,
            },
            Card {
                rank: Rank::King,
                suit: Suit::Hearts,
            },
        ],
        actions: vec![],
        board: vec![],
        pot: Some(Money {
            amount: 0.04,
            currency: Currency::USD,
        }),
        rake: None,
        result: HandResult {
            winners: vec![Winner {
                player: "Hero".to_string(),
                amount: Money {
                    amount: 0.04,
                    currency: Currency::USD,
                },
                pot: "main pot".to_string(),
            }],
            hero_result: HeroResult::Won,
        },
        raw_text: String::new(),
        stud_cards: None,
    }
}

fn make_test_embeddings() -> HandEmbeddings {
    HandEmbeddings {
        summary: vec![0.1; EMBEDDING_DIM as usize],
        action: vec![0.2; EMBEDDING_DIM as usize],
    }
}

#[tokio::test]
async fn test_new_creates_empty_table() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap(), "test")
        .await
        .unwrap();
    assert_eq!(store.count().await.unwrap(), 0);
}

#[tokio::test]
async fn test_upsert_and_get_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap(), "test")
        .await
        .unwrap();

    let hand = make_test_hand(12345);
    store
        .upsert_hand(
            &hand,
            "test summary",
            "PRE: HERO_OPEN(3bb)",
            make_test_embeddings(),
        )
        .await
        .unwrap();

    let retrieved = store.get_hand(12345).await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, 12345);
    assert_eq!(retrieved.hero, Some("Hero".to_string()));
}

#[tokio::test]
async fn test_hand_exists() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap(), "test")
        .await
        .unwrap();

    let hand = make_test_hand(99999);
    store
        .upsert_hand(&hand, "summary", "action", make_test_embeddings())
        .await
        .unwrap();

    assert!(store.hand_exists(99999).await.unwrap());
    assert!(!store.hand_exists(11111).await.unwrap());
}

#[tokio::test]
async fn test_scroll_with_filter() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap(), "test")
        .await
        .unwrap();

    let hand1 = make_test_hand(1);
    let mut hand2 = make_test_hand(2);
    hand2.game_type = GameType::Tournament {
        tournament_id: 100,
        level: 1,
        small_blind: Money {
            amount: 25.0,
            currency: Currency::Chips,
        },
        big_blind: Money {
            amount: 50.0,
            currency: Currency::Chips,
        },
        ante: None,
    };

    store
        .upsert_hand(&hand1, "cash hand", "action1", make_test_embeddings())
        .await
        .unwrap();
    store
        .upsert_hand(&hand2, "tourney hand", "action2", make_test_embeddings())
        .await
        .unwrap();

    let cash_hands = store
        .scroll_hands(Some("game_type = 'cash'".to_string()))
        .await
        .unwrap();
    assert_eq!(cash_hands.len(), 1);
    assert_eq!(cash_hands[0].id, 1);

    let all_hands = store.scroll_hands(None).await.unwrap();
    assert_eq!(all_hands.len(), 2);
}

#[tokio::test]
async fn test_count() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap(), "test")
        .await
        .unwrap();

    assert_eq!(store.count().await.unwrap(), 0);

    store
        .upsert_hand(&make_test_hand(1), "s1", "a1", make_test_embeddings())
        .await
        .unwrap();
    store
        .upsert_hand(&make_test_hand(2), "s2", "a2", make_test_embeddings())
        .await
        .unwrap();

    assert_eq!(store.count().await.unwrap(), 2);
}

#[tokio::test]
async fn test_upsert_dedup() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap(), "test")
        .await
        .unwrap();

    store
        .upsert_hand(&make_test_hand(1), "first", "a1", make_test_embeddings())
        .await
        .unwrap();
    store
        .upsert_hand(&make_test_hand(1), "updated", "a1", make_test_embeddings())
        .await
        .unwrap();

    assert_eq!(store.count().await.unwrap(), 1);
}
