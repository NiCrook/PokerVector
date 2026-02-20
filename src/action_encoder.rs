use std::collections::HashMap;

use crate::types::*;

/// Extract the big blind amount as f64 from a Hand.
fn big_blind_amount(hand: &Hand) -> f64 {
    match &hand.game_type {
        GameType::Cash { big_blind, .. } => big_blind.amount,
        GameType::Tournament { big_blind, .. } => big_blind.amount,
    }
}

/// Tracks pot size and per-player investment for accurate sizing calculations.
struct PotTracker {
    /// Total pot entering the current street
    pot_at_street_start: f64,
    /// Running pot within the current street
    current_pot: f64,
    /// Hero's total investment across all streets
    hero_invested: f64,
    /// Per-player investment in the current betting round (excludes antes)
    current_round: HashMap<String, f64>,
    /// The current bet/raise amount to call on this street
    current_bet: f64,
}

impl PotTracker {
    fn new() -> Self {
        Self {
            pot_at_street_start: 0.0,
            current_pot: 0.0,
            hero_invested: 0.0,
            current_round: HashMap::new(),
            current_bet: 0.0,
        }
    }

    fn new_street(&mut self) {
        self.pot_at_street_start = self.current_pot;
        self.current_round.clear();
        self.current_bet = 0.0;
    }

    fn process_action(&mut self, action: &Action, hero: &str) {
        let is_hero = action.player == hero;
        match &action.action_type {
            ActionType::PostSmallBlind { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::PostBigBlind { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                self.current_bet = amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::PostBlind { amount } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::PostAnte { amount } => {
                // Antes go to pot and hero_invested but NOT current_round
                let amt = amount.amount;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::BringsIn { amount } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                self.current_bet = amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::Call { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::Bet { amount, .. } => {
                let amt = amount.amount;
                *self.current_round.entry(action.player.clone()).or_default() += amt;
                self.current_pot += amt;
                self.current_bet = amt;
                if is_hero {
                    self.hero_invested += amt;
                }
            }
            ActionType::Raise { to, .. } => {
                let prev = self.current_round.get(&action.player).copied().unwrap_or(0.0);
                let increment = to.amount - prev;
                *self.current_round.entry(action.player.clone()).or_default() = to.amount;
                self.current_pot += increment;
                self.current_bet = to.amount;
                if is_hero {
                    self.hero_invested += increment;
                }
            }
            ActionType::UncalledBet { amount } => {
                let amt = amount.amount;
                self.current_pot -= amt;
                if is_hero {
                    self.hero_invested -= amt;
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
enum ActionLabel {
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
struct ClassifiedAction {
    label: ActionLabel,
    all_in: bool,
    alias: String,
}

/// Build a map from player name to anonymous alias (HERO, V1, V2, ...).
fn build_alias_map(hand: &Hand, hero: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Hero alias
    if let Some(ref h) = hand.hero {
        if h == hero {
            map.insert(h.clone(), "HERO".to_string());
        }
    }

    // Villains ordered by seat number
    let mut villains: Vec<&Player> = hand
        .players
        .iter()
        .filter(|p| !p.is_sitting_out && p.name != hero)
        .collect();
    villains.sort_by_key(|p| p.seat);

    for (i, v) in villains.iter().enumerate() {
        map.insert(v.name.clone(), format!("V{}", i + 1));
    }

    map
}

/// Format board cards visible at a given street.
fn format_board(board: &[Card], street: Street) -> String {
    match street {
        Street::Flop if board.len() >= 3 => {
            let cards: String = board[..3].iter().map(|c| c.to_string()).collect();
            format!("[{}]", cards)
        }
        Street::Turn if board.len() >= 4 => {
            format!("[{}]", board[3])
        }
        Street::River if board.len() >= 5 => {
            format!("[{}]", board[4])
        }
        _ => String::new(),
    }
}

/// Format a number with given decimal places, trimming trailing zeros and dot.
fn fmt_trimmed(val: f64, decimals: usize) -> String {
    let s = format!("{:.prec$}", val, prec = decimals);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Check if a street is a stud street.
fn is_stud_street(street: Street) -> bool {
    matches!(
        street,
        Street::ThirdStreet
            | Street::FourthStreet
            | Street::FifthStreet
            | Street::SixthStreet
            | Street::SeventhStreet
    )
}

/// Get the street label for encoding output.
fn street_label(street: Street) -> &'static str {
    match street {
        Street::Preflop => "PRE",
        Street::Flop => "FLOP",
        Street::Turn => "TURN",
        Street::River => "RIVER",
        Street::ThirdStreet => "3RD",
        Street::FourthStreet => "4TH",
        Street::FifthStreet => "5TH",
        Street::SixthStreet => "6TH",
        Street::SeventhStreet => "7TH",
        Street::Showdown => "SHOWDOWN",
    }
}

/// Encode a hand's action sequence into a structured text representation.
pub fn encode_action_sequence(hand: &Hand, hero: &str) -> String {
    // Sat out — single line
    if hand.result.hero_result == HeroResult::SatOut {
        return "SAT_OUT".to_string();
    }

    let bb = big_blind_amount(hand);
    let alias_map = build_alias_map(hand, hero);
    let is_stud = hand.variant == PokerVariant::SevenCardStud;

    let mut pot_tracker = PotTracker::new();
    let mut lines: Vec<String> = Vec::new();

    // Bomb pot prefix
    if hand.is_bomb_pot {
        lines.push("BOMB_POT".to_string());
    }

    // Determine street order
    let streets: &[Street] = if is_stud {
        &[
            Street::ThirdStreet,
            Street::FourthStreet,
            Street::FifthStreet,
            Street::SixthStreet,
            Street::SeventhStreet,
        ]
    } else {
        &[Street::Preflop, Street::Flop, Street::Turn, Street::River]
    };

    // Track preflop aggressor for c-bet detection
    let mut preflop_aggressor: Option<String> = None;

    for &street in streets {
        let street_actions: Vec<&Action> = hand
            .actions
            .iter()
            .filter(|a| a.street == street)
            .collect();

        if street_actions.is_empty() {
            continue;
        }

        // Signal new street to pot tracker (except first street)
        if street != streets[0] {
            pot_tracker.new_street();
        }

        let is_preflop = street == Street::Preflop;
        let mut raise_count: u32 = 0;
        let mut street_tokens: Vec<String> = Vec::new();
        let mut has_bet_on_street = false;

        for action in &street_actions {
            // Classify BEFORE updating pot tracker (so current_bet reflects pre-action state)
            let classified = classify_action(
                action,
                &alias_map,
                &pot_tracker,
                bb,
                is_preflop,
                is_stud_street(street),
                &mut raise_count,
                &mut has_bet_on_street,
                &preflop_aggressor,
            );

            if let Some(ca) = classified {
                // Track preflop aggressor
                if is_preflop {
                    match &ca.label {
                        ActionLabel::Open(_)
                        | ActionLabel::ThreeBet(_)
                        | ActionLabel::FourBet(_)
                        | ActionLabel::FiveBetPlus(_) => {
                            preflop_aggressor = Some(action.player.clone());
                        }
                        _ => {}
                    }
                }

                street_tokens.push(format_classified_action(&ca));
            }

            // Update pot tracker AFTER classification
            pot_tracker.process_action(action, hero);
        }

        if street_tokens.is_empty() {
            continue;
        }

        // Walk detection: hero is BB, won, and only folds happened preflop
        if is_preflop && is_walk(hand, hero, &street_actions) {
            let label = street_label(street);
            lines.push(format!("{}: WALK", label));
            continue;
        }

        // Build street line
        let label = street_label(street);
        let board_str = if !is_stud {
            format_board(&hand.board, street)
        } else {
            String::new()
        };

        if board_str.is_empty() {
            lines.push(format!("{}: {}", label, street_tokens.join(" ")));
        } else {
            lines.push(format!("{}{}: {}", label, board_str, street_tokens.join(" ")));
        }
    }

    // Result line
    let result_line = build_result_line(hand, hero, bb, &pot_tracker);
    if let Some(rl) = result_line {
        lines.push(rl);
    }

    lines.join("\n")
}

/// Classify a single action into an ActionLabel with context.
/// Returns None for actions that should not be encoded (blinds, antes, etc.).
fn classify_action(
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
                // Track aggressor (handled in caller)
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
fn format_classified_action(ca: &ClassifiedAction) -> String {
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

/// Detect walk: hero is BB, hero won, and no voluntary actions (call/bet/raise) preflop.
fn is_walk(hand: &Hand, hero: &str, preflop_actions: &[&Action]) -> bool {
    if hand.result.hero_result != HeroResult::Won {
        return false;
    }

    // Check hero is BB
    let hero_is_bb = hand
        .hero_position
        .map(|p| p == Position::BB)
        .unwrap_or(false);
    if !hero_is_bb {
        return false;
    }

    // Check no voluntary actions exist (only blinds, antes, and folds)
    !preflop_actions.iter().any(|a| {
        matches!(
            a.action_type,
            ActionType::Call { .. } | ActionType::Bet { .. } | ActionType::Raise { .. }
        )
    })
}

/// Build the RESULT line for the encoding.
fn build_result_line(hand: &Hand, hero: &str, bb: f64, pot_tracker: &PotTracker) -> Option<String> {
    match hand.result.hero_result {
        HeroResult::SatOut => None,
        HeroResult::Won => {
            let collected: f64 = hand
                .result
                .winners
                .iter()
                .filter(|w| w.player == hero)
                .map(|w| w.amount.amount)
                .sum();
            let net = (collected - pot_tracker.hero_invested) / bb;
            Some(format!("RESULT: HERO({}bb)", fmt_result(net)))
        }
        HeroResult::Lost | HeroResult::Folded => {
            let net = -pot_tracker.hero_invested / bb;
            Some(format!("RESULT: HERO({}bb)", fmt_result(net)))
        }
    }
}

/// Format a net result in BB with sign prefix.
fn fmt_result(net: f64) -> String {
    let formatted = fmt_trimmed(net.abs(), 1);
    if net >= 0.0 {
        format!("+{}", formatted)
    } else {
        format!("-{}", formatted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card(r: char, s: char) -> Card {
        Card {
            rank: Rank::from_char(r).unwrap(),
            suit: Suit::from_char(s).unwrap(),
        }
    }

    fn usd(amount: f64) -> Money {
        Money {
            amount,
            currency: Currency::USD,
        }
    }

    fn make_player(seat: u8, name: &str, is_hero: bool) -> Player {
        Player {
            seat,
            name: name.to_string(),
            stack: usd(100.0),
            position: None,
            is_hero,
            is_sitting_out: false,
        }
    }

    fn make_base_hand() -> Hand {
        Hand {
            id: 12345,
            site: Site::ACR,
            variant: PokerVariant::Holdem,
            betting_limit: BettingLimit::NoLimit,
            is_hi_lo: false,
            is_bomb_pot: false,
            game_type: GameType::Cash {
                small_blind: usd(0.01),
                big_blind: usd(0.02),
                ante: None,
            },
            timestamp: "2024-01-01 12:00:00".to_string(),
            table_name: "Test".to_string(),
            table_size: 6,
            button_seat: 1,
            players: vec![
                make_player(1, "V_BTN", false),
                make_player(2, "V_SB", false),
                make_player(3, "Hero", true),
                make_player(4, "V_CO", false),
            ],
            hero: Some("Hero".to_string()),
            hero_position: Some(Position::BB),
            hero_cards: vec![make_card('A', 's'), make_card('K', 'h')],
            actions: vec![],
            board: vec![],
            pot: Some(usd(0.10)),
            rake: None,
            result: HandResult {
                winners: vec![],
                hero_result: HeroResult::Folded,
            },
            raw_text: String::new(),
            stud_cards: None,
        }
    }

    #[test]
    fn test_encode_simple_srp() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            // Blinds
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            // BTN opens
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            // SB folds
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            // BB calls
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
            // Flop
            Action { player: "Hero".to_string(), action_type: ActionType::Check, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Bet { amount: usd(0.08), all_in: false }, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Flop },
        ];
        hand.board = vec![
            make_card('A', 'h'), make_card('7', 'c'), make_card('2', 'd'),
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Villain".to_string(), amount: usd(0.20), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Folded,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        // Verify preflop uses BB sizing
        assert!(encoded.contains("V1_OPEN(3bb)"), "Expected OPEN(3bb), got:\n{}", encoded);
        // Verify call sizing in bb
        assert!(encoded.contains("HERO_CALL(2bb)"), "Expected CALL(2bb), got:\n{}", encoded);
        // Flop uses pot fractions
        assert!(encoded.contains("FLOP[Ah7c2d]"), "Expected FLOP[Ah7c2d], got:\n{}", encoded);
        // V1 bets — should be c-bet since V1 was preflop aggressor
        assert!(encoded.contains("V1_CBET("), "Expected V1_CBET, got:\n{}", encoded);
        // Hero folds on flop
        assert!(encoded.contains("HERO_FOLD"), "Expected HERO_FOLD, got:\n{}", encoded);
        // Result
        assert!(encoded.contains("RESULT: HERO(-3bb)"), "Expected RESULT: HERO(-3bb), got:\n{}", encoded);
    }

    #[test]
    fn test_encode_3bet_pot() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            // V opens
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            // Hero 3-bets
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: usd(0.16), to: usd(0.18), all_in: false }, street: Street::Preflop },
            // V calls
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: usd(0.12), all_in: false }, street: Street::Preflop },
        ];
        hand.board = vec![
            make_card('K', 's'), make_card('9', 'h'), make_card('3', 'd'),
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: usd(0.37), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        assert!(encoded.contains("V1_OPEN(3bb)"), "got:\n{}", encoded);
        assert!(encoded.contains("HERO_3BET(9bb)"), "got:\n{}", encoded);
        assert!(encoded.contains("V1_CALL(6bb)"), "got:\n{}", encoded);
    }

    #[test]
    fn test_encode_4bet_pot() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: usd(0.16), to: usd(0.18), all_in: false }, street: Street::Preflop },
            // V 4-bets
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.26), to: usd(0.44), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.26), all_in: false }, street: Street::Preflop },
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: usd(0.89), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        assert!(encoded.contains("V1_OPEN(3bb)"), "got:\n{}", encoded);
        assert!(encoded.contains("HERO_3BET(9bb)"), "got:\n{}", encoded);
        assert!(encoded.contains("V1_4BET(22bb)"), "got:\n{}", encoded);
        assert!(encoded.contains("HERO_CALL(13bb)"), "got:\n{}", encoded);
    }

    #[test]
    fn test_encode_multiway() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "P1", false),
            make_player(2, "P2", false),
            make_player(3, "Hero", true),
            make_player(4, "P4", false),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "P2".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "P4".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "P1".to_string(), action_type: ActionType::Call { amount: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "P2".to_string(), action_type: ActionType::Call { amount: usd(0.05), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
        ];
        hand.result = HandResult {
            winners: vec![],
            hero_result: HeroResult::Folded,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        // All player aliases should appear
        assert!(encoded.contains("V3_OPEN"), "got:\n{}", encoded);
        assert!(encoded.contains("V1_CALL"), "got:\n{}", encoded);
        assert!(encoded.contains("V2_CALL"), "got:\n{}", encoded);
        assert!(encoded.contains("HERO_CALL"), "got:\n{}", encoded);
    }

    #[test]
    fn test_encode_stakes_normalization() {
        // Two hands with identical structure but different stakes
        let make_hand_at_stakes = |sb: f64, bb_amt: f64| -> Hand {
            let mut hand = make_base_hand();
            hand.game_type = GameType::Cash {
                small_blind: usd(sb),
                big_blind: usd(bb_amt),
                ante: None,
            };
            hand.players = vec![
                make_player(1, "Villain", false),
                make_player(2, "SBPlayer", false),
                make_player(3, "Hero", true),
            ];
            hand.hero_position = Some(Position::BB);
            // Same structure: open to 3bb, hero calls
            hand.actions = vec![
                Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(sb), all_in: false }, street: Street::Preflop },
                Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(bb_amt), all_in: false }, street: Street::Preflop },
                Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(bb_amt * 2.0), to: usd(bb_amt * 3.0), all_in: false }, street: Street::Preflop },
                Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
                Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(bb_amt * 2.0), all_in: false }, street: Street::Preflop },
            ];
            hand.result = HandResult {
                winners: vec![Winner { player: "Hero".to_string(), amount: usd(bb_amt * 6.5), pot: "Main pot".to_string() }],
                hero_result: HeroResult::Won,
            };
            hand
        };

        let hand_micro = make_hand_at_stakes(0.01, 0.02);
        let hand_mid = make_hand_at_stakes(1.0, 2.0);

        let enc_micro = encode_action_sequence(&hand_micro, "Hero");
        let enc_mid = encode_action_sequence(&hand_mid, "Hero");

        assert_eq!(enc_micro, enc_mid, "Same structure at different stakes should produce identical output:\nmicro: {}\nmid: {}", enc_micro, enc_mid);
    }

    #[test]
    fn test_encode_bomb_pot() {
        let mut hand = make_base_hand();
        hand.is_bomb_pot = true;
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "Hero", true),
        ];
        hand.actions = vec![
            // No preflop actions in bomb pots — play starts on flop
            Action { player: "Hero".to_string(), action_type: ActionType::Check, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Bet { amount: usd(0.10), all_in: false }, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Flop },
        ];
        hand.board = vec![
            make_card('T', 's'), make_card('8', 'h'), make_card('3', 'c'),
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Villain".to_string(), amount: usd(0.50), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Folded,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        assert!(encoded.starts_with("BOMB_POT\n"), "Expected BOMB_POT prefix, got:\n{}", encoded);
        assert!(!encoded.contains("PRE:"), "Bomb pots should have no preflop line, got:\n{}", encoded);
        assert!(encoded.contains("FLOP[Ts8h3c]"), "got:\n{}", encoded);
    }

    #[test]
    fn test_encode_stud() {
        let mut hand = make_base_hand();
        hand.variant = PokerVariant::SevenCardStud;
        hand.betting_limit = BettingLimit::FixedLimit;
        hand.is_hi_lo = true;
        hand.game_type = GameType::Cash {
            small_blind: usd(0.04),
            big_blind: usd(0.08),
            ante: Some(usd(0.01)),
        };
        hand.button_seat = 0; // no button in stud
        hand.players = vec![
            make_player(1, "Hero", true),
            make_player(2, "StudV", false),
        ];
        hand.hero_position = None; // no positions in stud
        hand.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::PostAnte { amount: usd(0.01) }, street: Street::ThirdStreet },
            Action { player: "StudV".to_string(), action_type: ActionType::PostAnte { amount: usd(0.01) }, street: Street::ThirdStreet },
            Action { player: "StudV".to_string(), action_type: ActionType::BringsIn { amount: usd(0.02) }, street: Street::ThirdStreet },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.02), all_in: false }, street: Street::ThirdStreet },
            // 4th street
            Action { player: "Hero".to_string(), action_type: ActionType::Check, street: Street::FourthStreet },
            Action { player: "StudV".to_string(), action_type: ActionType::Bet { amount: usd(0.04), all_in: false }, street: Street::FourthStreet },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::FourthStreet },
        ];
        hand.board = vec![]; // no community board in stud
        hand.result = HandResult {
            winners: vec![Winner { player: "StudV".to_string(), amount: usd(0.10), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Folded,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        assert!(encoded.contains("3RD:"), "Expected 3RD: label, got:\n{}", encoded);
        assert!(encoded.contains("BRINGIN"), "Expected BRINGIN, got:\n{}", encoded);
        assert!(!encoded.contains("["), "Stud should have no board cards, got:\n{}", encoded);
        assert!(encoded.contains("4TH:"), "Expected 4TH: label, got:\n{}", encoded);
    }

    #[test]
    fn test_encode_all_in() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.28), to: usd(0.30), all_in: true }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.28), all_in: true }, street: Street::Preflop },
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: usd(0.61), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        assert!(encoded.contains("_AI("), "Expected _AI suffix, got:\n{}", encoded);
        assert!(encoded.contains("V1_OPEN_AI(15bb)"), "got:\n{}", encoded);
        assert!(encoded.contains("HERO_CALL_AI(14bb)"), "got:\n{}", encoded);
    }

    #[test]
    fn test_encode_walk() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: usd(0.03), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        assert!(encoded.contains("PRE: WALK"), "Expected WALK, got:\n{}", encoded);
        assert!(encoded.contains("RESULT: HERO(+0.5bb)"), "Expected +0.5bb result, got:\n{}", encoded);
    }

    #[test]
    fn test_encode_sat_out() {
        let mut hand = make_base_hand();
        hand.result.hero_result = HeroResult::SatOut;
        hand.actions = vec![];

        let encoded = encode_action_sequence(&hand, "Hero");
        assert_eq!(encoded, "SAT_OUT");
    }

    #[test]
    fn test_pot_tracker_accuracy() {
        // Multi-street hand: verify pot amounts
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            // Preflop: SB 0.01, BB 0.02, V raises to 0.06, SB folds, Hero calls 0.04
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
            // Pot at flop start: 0.01 + 0.02 + 0.06 + 0.04 = 0.13
            // Flop: Hero checks, V bets 0.08 (0.62pot), Hero calls 0.08
            Action { player: "Hero".to_string(), action_type: ActionType::Check, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Bet { amount: usd(0.08), all_in: false }, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.08), all_in: false }, street: Street::Flop },
            // Pot at turn start: 0.13 + 0.08 + 0.08 = 0.29
            Action { player: "Hero".to_string(), action_type: ActionType::Check, street: Street::Turn },
            Action { player: "Villain".to_string(), action_type: ActionType::Check, street: Street::Turn },
        ];
        hand.board = vec![
            make_card('A', 'h'), make_card('7', 'c'), make_card('2', 'd'),
            make_card('5', 's'),
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: usd(0.29), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        // The flop c-bet should be relative to pot_at_street_start (0.13)
        // 0.08 / 0.13 ≈ 0.62
        assert!(encoded.contains("V1_CBET(0.62pot)"), "Expected CBET(0.62pot), got:\n{}", encoded);
        // The call on flop: 0.08 / 0.13 ≈ 0.62
        assert!(encoded.contains("HERO_CALL(0.62pot)"), "Expected CALL(0.62pot), got:\n{}", encoded);
    }

    #[test]
    fn test_postflop_raise_multiplier() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
            // Flop: pot = 0.13. Hero bets 0.10, V raises to 0.30 (3x)
            Action { player: "Hero".to_string(), action_type: ActionType::Bet { amount: usd(0.10), all_in: false }, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.20), to: usd(0.30), all_in: false }, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Flop },
        ];
        hand.board = vec![
            make_card('K', 's'), make_card('9', 'h'), make_card('3', 'd'),
        ];
        hand.result = HandResult {
            winners: vec![Winner { player: "Villain".to_string(), amount: usd(0.53), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Folded,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        // Hero bets first on flop but is NOT preflop aggressor (Villain opened) — so it's a BET not CBET
        // Wait — Hero called preflop, Villain opened, so Villain is preflop aggressor.
        // Hero bets first on flop → this is a donk bet, not a cbet.
        assert!(encoded.contains("HERO_BET("), "Expected HERO_BET, got:\n{}", encoded);
        // Villain raises to 0.30, current_bet was 0.10 → 3x multiplier
        assert!(encoded.contains("V1_RAISE(3x)"), "Expected V1_RAISE(3x), got:\n{}", encoded);
    }

    #[test]
    fn test_uncalled_bet_adjusts_pot() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
            // Flop: Hero bets 0.10, V folds → uncalled bet returned
            Action { player: "Hero".to_string(), action_type: ActionType::Bet { amount: usd(0.10), all_in: false }, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::UncalledBet { amount: usd(0.10) }, street: Street::Flop },
        ];
        hand.board = vec![
            make_card('A', 'h'), make_card('7', 'c'), make_card('2', 'd'),
        ];
        // Hero wins the pot (0.13, since uncalled 0.10 was returned)
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: usd(0.13), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        // Net should be: collected 0.13 - invested (0.02 + 0.04) = +0.07 / 0.02 = +3.5bb
        // hero_invested: 0.02 (BB) + 0.04 (call) + 0.10 (bet) - 0.10 (uncalled) = 0.06
        assert!(encoded.contains("RESULT: HERO(+3.5bb)"), "Expected +3.5bb, got:\n{}", encoded);
    }

    #[test]
    fn test_net_result_calculation() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
        ];
        // Hero invested: 0.02 + 0.04 = 0.06
        // Hero wins 0.13 → net = 0.07 / 0.02 = +3.5bb
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: usd(0.13), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        assert!(encoded.contains("RESULT: HERO(+3.5bb)"), "Expected +3.5bb, got:\n{}", encoded);
    }

    #[test]
    fn test_call_sizing_preflop() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
        ];
        hand.result = HandResult {
            winners: vec![],
            hero_result: HeroResult::Folded,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        // Call of 0.04 / 0.02 (bb) = 2bb
        assert!(encoded.contains("HERO_CALL(2bb)"), "Expected CALL(2bb), got:\n{}", encoded);
    }

    #[test]
    fn test_call_sizing_postflop() {
        let mut hand = make_base_hand();
        hand.players = vec![
            make_player(1, "Villain", false),
            make_player(2, "SBPlayer", false),
            make_player(3, "Hero", true),
        ];
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "SBPlayer".to_string(), action_type: ActionType::PostSmallBlind { amount: usd(0.01), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: usd(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: usd(0.04), to: usd(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "SBPlayer".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.04), all_in: false }, street: Street::Preflop },
            // Flop: pot = 0.13, V bets 0.08, Hero calls 0.08
            Action { player: "Villain".to_string(), action_type: ActionType::Bet { amount: usd(0.08), all_in: false }, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: usd(0.08), all_in: false }, street: Street::Flop },
        ];
        hand.board = vec![
            make_card('A', 'h'), make_card('7', 'c'), make_card('2', 'd'),
        ];
        hand.result = HandResult {
            winners: vec![],
            hero_result: HeroResult::Folded,
        };

        let encoded = encode_action_sequence(&hand, "Hero");
        // Call of 0.08 / 0.13 (pot at street start) ≈ 0.62pot
        assert!(encoded.contains("HERO_CALL(0.62pot)"), "Expected CALL(0.62pot), got:\n{}", encoded);
    }
}
