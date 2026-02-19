use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PokerVariant {
    Holdem,
    Omaha,          // 4-card
    FiveCardOmaha,  // 5-card
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
    PostSmallBlind { amount: Money, all_in: bool },
    PostBigBlind { amount: Money, all_in: bool },
    PostAnte { amount: Money },
    PostBlind { amount: Money },
    Fold,
    Check,
    Call { amount: Money, all_in: bool },
    Bet { amount: Money, all_in: bool },
    Raise { amount: Money, to: Money, all_in: bool },
    UncalledBet { amount: Money },
    Shows { cards: Vec<Option<Card>>, description: Option<String> },
    DoesNotShow,
    Mucks,
    SitsOut,
    WaitsForBigBlind,
    Collected { amount: Money, pot: String },
    BringsIn { amount: Money },
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
