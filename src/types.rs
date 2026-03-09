use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PokerVariant {
    Holdem,
    Omaha,         // 4-card
    FiveCardOmaha, // 5-card
    SevenCardStud,
}

impl fmt::Display for PokerVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PokerVariant::Holdem => "Hold'em",
            PokerVariant::Omaha => "Omaha",
            PokerVariant::FiveCardOmaha => "5-Card Omaha",
            PokerVariant::SevenCardStud => "7-Card Stud",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BettingLimit {
    NoLimit,
    PotLimit,
    FixedLimit,
}

impl fmt::Display for BettingLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BettingLimit::NoLimit => "No Limit",
            BettingLimit::PotLimit => "Pot Limit",
            BettingLimit::FixedLimit => "Fixed Limit",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudPlayerCards {
    pub player: String,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    pub fn from_char(c: char) -> Option<Rank> {
        match c {
            '2' => Some(Rank::Two),
            '3' => Some(Rank::Three),
            '4' => Some(Rank::Four),
            '5' => Some(Rank::Five),
            '6' => Some(Rank::Six),
            '7' => Some(Rank::Seven),
            '8' => Some(Rank::Eight),
            '9' => Some(Rank::Nine),
            'T' => Some(Rank::Ten),
            'J' => Some(Rank::Jack),
            'Q' => Some(Rank::Queen),
            'K' => Some(Rank::King),
            'A' => Some(Rank::Ace),
            _ => None,
        }
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = match self {
            Rank::Two => '2',
            Rank::Three => '3',
            Rank::Four => '4',
            Rank::Five => '5',
            Rank::Six => '6',
            Rank::Seven => '7',
            Rank::Eight => '8',
            Rank::Nine => '9',
            Rank::Ten => 'T',
            Rank::Jack => 'J',
            Rank::Queen => 'Q',
            Rank::King => 'K',
            Rank::Ace => 'A',
        };
        write!(f, "{}", c)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub fn from_char(c: char) -> Option<Suit> {
        match c {
            'c' => Some(Suit::Clubs),
            'd' => Some(Suit::Diamonds),
            'h' => Some(Suit::Hearts),
            's' => Some(Suit::Spades),
            _ => None,
        }
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = match self {
            Suit::Clubs => 'c',
            Suit::Diamonds => 'd',
            Suit::Hearts => 'h',
            Suit::Spades => 's',
        };
        write!(f, "{}", c)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank, self.suit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Currency {
    USD,
    Chips,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Money {
    pub amount: f64,
    pub currency: Currency,
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.currency {
            Currency::USD => write!(f, "${:.2}", self.amount),
            Currency::Chips => write!(f, "{:.0}", self.amount),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    BTN,
    SB,
    BB,
    CO,
    HJ,
    LJ,
    MP2,
    MP1,
    UTG,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Position::BTN => "BTN",
            Position::SB => "SB",
            Position::BB => "BB",
            Position::CO => "CO",
            Position::HJ => "HJ",
            Position::LJ => "LJ",
            Position::MP2 => "MP2",
            Position::MP1 => "MP1",
            Position::UTG => "UTG",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Site {
    ACR,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameType {
    Cash {
        small_blind: Money,
        big_blind: Money,
        ante: Option<Money>,
    },
    Tournament {
        tournament_id: u64,
        level: u32,
        small_blind: Money,
        big_blind: Money,
        ante: Option<Money>,
    },
}

impl fmt::Display for GameType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameType::Cash {
                small_blind,
                big_blind,
                ante,
            } => {
                write!(f, "Cash {}/{}", small_blind, big_blind)?;
                if let Some(a) = ante {
                    write!(f, " ante {}", a)?;
                }
                Ok(())
            }
            GameType::Tournament {
                tournament_id,
                level,
                small_blind,
                big_blind,
                ante,
            } => {
                write!(
                    f,
                    "Tournament #{} L{} {}/{}",
                    tournament_id, level, small_blind, big_blind
                )?;
                if let Some(a) = ante {
                    write!(f, " ante {}", a)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub seat: u8,
    pub name: String,
    pub stack: Money,
    pub position: Option<Position>,
    pub is_hero: bool,
    pub is_sitting_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
    ThirdStreet,
    FourthStreet,
    FifthStreet,
    SixthStreet,
    SeventhStreet,
    Showdown,
}

impl fmt::Display for Street {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Street::Preflop => "Preflop",
            Street::Flop => "Flop",
            Street::Turn => "Turn",
            Street::River => "River",
            Street::ThirdStreet => "3rd Street",
            Street::FourthStreet => "4th Street",
            Street::FifthStreet => "5th Street",
            Street::SixthStreet => "6th Street",
            Street::SeventhStreet => "7th Street",
            Street::Showdown => "Showdown",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub player: String,
    pub action_type: ActionType,
    pub street: Street,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionType {
    PostSmallBlind {
        amount: Money,
        all_in: bool,
    },
    PostBigBlind {
        amount: Money,
        all_in: bool,
    },
    PostAnte {
        amount: Money,
    },
    PostBlind {
        amount: Money,
    },
    Fold,
    Check,
    Call {
        amount: Money,
        all_in: bool,
    },
    Bet {
        amount: Money,
        all_in: bool,
    },
    Raise {
        amount: Money,
        to: Money,
        all_in: bool,
    },
    UncalledBet {
        amount: Money,
    },
    Shows {
        cards: Vec<Option<Card>>,
        description: Option<String>,
    },
    DoesNotShow,
    Mucks,
    SitsOut,
    WaitsForBigBlind,
    Collected {
        amount: Money,
        pot: String,
    },
    BringsIn {
        amount: Money,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Winner {
    pub player: String,
    pub amount: Money,
    pub pot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeroResult {
    Won,
    Lost,
    Folded,
    SatOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandResult {
    pub winners: Vec<Winner>,
    pub hero_result: HeroResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hand {
    pub id: u64,
    pub site: Site,
    pub variant: PokerVariant,
    pub betting_limit: BettingLimit,
    pub is_hi_lo: bool,
    pub is_bomb_pot: bool,
    pub game_type: GameType,
    pub timestamp: String,
    pub table_name: String,
    pub table_size: u8,
    pub button_seat: u8,
    pub players: Vec<Player>,
    pub hero: Option<String>,
    pub hero_position: Option<Position>,
    pub hero_cards: Vec<Card>,
    pub actions: Vec<Action>,
    pub board: Vec<Card>,
    pub pot: Option<Money>,
    pub rake: Option<Money>,
    pub result: HandResult,
    pub raw_text: String,
    pub stud_cards: Option<Vec<StudPlayerCards>>,
}

impl Hand {
    /// Compact JSON for token-efficient MCP responses.
    ///
    /// Omits: raw_text, currency, site, button_seat, is_hero, is_sitting_out.
    /// Groups actions by street, strips blind post amounts and call amounts
    /// (except all-in calls), removes uncalled bets and collected actions.
    pub fn to_compact(&self) -> serde_json::Value {
        // Variant + limit label
        let variant_str = format!("{} {}", self.betting_limit, self.variant);

        // Stakes string
        let stakes_str = match &self.game_type {
            GameType::Cash {
                small_blind,
                big_blind,
                ante,
            } => {
                let base = format!("{}/{}", small_blind.amount, big_blind.amount);
                match ante {
                    Some(a) => format!("{} ante {}", base, a.amount),
                    None => base,
                }
            }
            GameType::Tournament {
                tournament_id,
                level,
                small_blind,
                big_blind,
                ante,
            } => {
                let base = format!(
                    "T#{} L{} {}/{}",
                    tournament_id, level, small_blind.amount, big_blind.amount
                );
                match ante {
                    Some(a) => format!("{} ante {}", base, a.amount),
                    None => base,
                }
            }
        };

        // Players as compact tuples [name, position, stack]
        let players: Vec<serde_json::Value> = self
            .players
            .iter()
            .filter(|p| !p.is_sitting_out)
            .map(|p| {
                json!([
                    p.name,
                    p.position.map(|pos| pos.to_string()).unwrap_or_default(),
                    p.stack.amount,
                ])
            })
            .collect();

        // Hero cards as compact strings
        let hero_cards: Vec<String> = self.hero_cards.iter().map(|c| c.to_string()).collect();

        // Group actions by street, format as compact strings
        let street_order: &[Street] = if self.variant == PokerVariant::SevenCardStud {
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

        let mut street_actions: Vec<(&str, Vec<String>)> = Vec::new();

        for &street in street_order {
            let actions: Vec<String> = self
                .actions
                .iter()
                .filter(|a| a.street == street)
                .filter_map(|a| Self::compact_action(a))
                .collect();

            if !actions.is_empty() {
                let key = match street {
                    Street::Preflop => "preflop",
                    Street::Flop => "flop",
                    Street::Turn => "turn",
                    Street::River => "river",
                    Street::ThirdStreet => "3rd",
                    Street::FourthStreet => "4th",
                    Street::FifthStreet => "5th",
                    Street::SixthStreet => "6th",
                    Street::SeventhStreet => "7th",
                    Street::Showdown => "showdown",
                };
                street_actions.push((key, actions));
            }
        }

        // Showdown actions (shows)
        let showdown: Vec<String> = self
            .actions
            .iter()
            .filter(|a| a.street == Street::Showdown)
            .filter_map(|a| Self::compact_action(a))
            .collect();
        if !showdown.is_empty() {
            street_actions.push(("showdown", showdown));
        }

        // Board cards, split by street
        let board_str = if self.board.is_empty() {
            None
        } else {
            let parts: Vec<String> = self.board.iter().map(|c| c.to_string()).collect();
            Some(match parts.len() {
                3 => format!("[{}]", parts.join(" ")),
                4 => format!("[{}] [{}]", parts[..3].join(" "), parts[3]),
                5 => format!("[{}] [{}] [{}]", parts[..3].join(" "), parts[3], parts[4]),
                _ => parts.join(" "),
            })
        };

        // Result string
        let result_str = match &self.result.hero_result {
            HeroResult::Won => {
                let total: f64 = self
                    .result
                    .winners
                    .iter()
                    .filter(|w| Some(&w.player) == self.hero.as_ref())
                    .map(|w| w.amount.amount)
                    .sum();
                format!("Won {:.2}", total)
            }
            HeroResult::Lost => "Lost".to_string(),
            HeroResult::Folded => "Folded".to_string(),
            HeroResult::SatOut => "SatOut".to_string(),
        };

        // Build the JSON object
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), json!(self.id));
        obj.insert("variant".into(), json!(variant_str));
        if self.is_hi_lo {
            obj.insert("hi_lo".into(), json!(true));
        }
        if self.is_bomb_pot {
            obj.insert("bomb_pot".into(), json!(true));
        }
        obj.insert("stakes".into(), json!(stakes_str));
        obj.insert("timestamp".into(), json!(self.timestamp));
        obj.insert(
            "table".into(),
            json!(format!("{} {}-max", self.table_name, self.table_size)),
        );
        if let Some(ref hero) = self.hero {
            obj.insert("hero".into(), json!(hero));
        }
        if let Some(pos) = self.hero_position {
            obj.insert("hero_position".into(), json!(pos.to_string()));
        }
        if !hero_cards.is_empty() {
            obj.insert("hero_cards".into(), json!(hero_cards));
        }
        obj.insert("players".into(), json!(players));

        // Street actions
        for (key, actions) in &street_actions {
            obj.insert((*key).to_string(), json!(actions));
        }

        if let Some(board) = board_str {
            obj.insert("board".into(), json!(board));
        }

        obj.insert(
            "pot".into(),
            json!(self.pot.map(|p| p.amount).unwrap_or(0.0)),
        );
        obj.insert("result".into(), json!(result_str));

        // Stud cards if present
        if let Some(ref stud) = self.stud_cards {
            let sc: Vec<serde_json::Value> = stud
                .iter()
                .map(|sc| {
                    let cards: Vec<String> = sc.cards.iter().map(|c| c.to_string()).collect();
                    json!([sc.player, cards.join(" ")])
                })
                .collect();
            obj.insert("stud_cards".into(), json!(sc));
        }

        serde_json::Value::Object(obj)
    }

    /// Format a single action as a compact string.
    /// Returns None for actions that should be omitted (blinds, uncalled bets, collected).
    pub fn compact_action(action: &Action) -> Option<String> {
        let name = &action.player;
        match &action.action_type {
            // Omit blind posts, antes, uncalled bets, collected — derivable or bookkeeping
            ActionType::PostSmallBlind { .. }
            | ActionType::PostBigBlind { .. }
            | ActionType::PostAnte { .. }
            | ActionType::PostBlind { .. }
            | ActionType::UncalledBet { .. }
            | ActionType::Collected { .. }
            | ActionType::SitsOut
            | ActionType::WaitsForBigBlind => None,

            ActionType::Fold => Some(format!("{} folds", name)),
            ActionType::Check => Some(format!("{} checks", name)),
            ActionType::Call { all_in: true, .. } => Some(format!("{} calls (all-in)", name)),
            ActionType::Call { .. } => Some(format!("{} calls", name)),
            ActionType::Bet { amount, all_in } => {
                let ai = if *all_in { " (all-in)" } else { "" };
                Some(format!("{} bets {:.2}{}", name, amount.amount, ai))
            }
            ActionType::Raise { to, all_in, .. } => {
                let ai = if *all_in { " (all-in)" } else { "" };
                Some(format!("{} raises to {:.2}{}", name, to.amount, ai))
            }
            ActionType::BringsIn { amount } => {
                Some(format!("{} brings in {:.2}", name, amount.amount))
            }
            ActionType::Shows {
                cards, description, ..
            } => {
                let card_str: String = cards
                    .iter()
                    .map(|c| match c {
                        Some(c) => c.to_string(),
                        None => "?".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                match description {
                    Some(d) => Some(format!("{} shows [{}] ({})", name, card_str, d)),
                    None => Some(format!("{} shows [{}]", name, card_str)),
                }
            }
            ActionType::DoesNotShow => Some(format!("{} does not show", name)),
            ActionType::Mucks => Some(format!("{} mucks", name)),
        }
    }
}
