use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchHandsParams {
    #[schemars(description = "Natural language search query (e.g. 'hero 3-bets from the button')")]
    pub query: String,
    #[schemars(description = "Search mode: 'semantic' (default, matches narrative descriptions) or 'action' (matches betting line structure)")]
    pub search_mode: Option<String>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by villain name")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by result: won, lost, folded")]
    pub result: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Max results to return (default 10)")]
    pub limit: Option<u64>,
    #[schemars(description = "Offset for pagination (default 0). Use with limit to page through results.")]
    pub offset: Option<u64>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15' or '2024-01-15 00:00:00')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15' or '2024-02-15 23:59:59')")]
    pub to_date: Option<String>,
    #[schemars(description = "Filter by user-applied tag (e.g. 'bad call', 'review later')")]
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSimilarParams {
    #[schemars(description = "Hand ID to find similar hands for")]
    pub hand_id: u64,
    #[schemars(description = "Similarity mode: 'action' (default, matches betting structure), 'semantic' (matches narrative)")]
    pub mode: Option<String>,
    #[schemars(description = "Max results (default 10)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetHandParams {
    #[schemars(description = "The hand ID to retrieve")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetStatsParams {
    #[schemars(description = "Player name to compute stats for (defaults to hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by villain name")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListVillainsParams {
    #[schemars(description = "Minimum number of hands to include a villain (default 10)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TableProfitabilityParams {
    #[schemars(description = "Group by 'stakes' (default) or 'table'")]
    pub group_by: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Minimum hands per group to include in results (default 1)")]
    pub min_hands: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BestVillainsParams {
    #[schemars(description = "Minimum hands played against villain (default 10)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Max results to return (default 10)")]
    pub limit: Option<u64>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorstVillainsParams {
    #[schemars(description = "Minimum hands played against villain (default 10)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Max results to return (default 10)")]
    pub limit: Option<u64>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListSessionsParams {
    #[schemars(description = "Max sessions to return (default 20, most recent first)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReviewSessionParams {
    #[schemars(description = "Session ID from list_sessions output")]
    pub session_id: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WatchDirectoryParams {
    #[schemars(description = "Override path to import from (default: all configured accounts)")]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetLastImportParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AutoTagHandsParams {
    #[schemars(description = "Big blind threshold for 'big' tags (default 20)")]
    pub min_pot_bb: Option<f64>,
    #[schemars(description = "Max hands per category (default 10)")]
    pub limit: Option<u64>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCoolersParams {
    #[schemars(description = "Minimum pot size in big blinds (default 30)")]
    pub min_pot_bb: Option<f64>,
    #[schemars(description = "Max results to return (default 20)")]
    pub limit: Option<u64>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetBluffCandidatesParams {
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Minimum pot size in big blinds at the time hero folded (default 3)")]
    pub min_pot_bb: Option<f64>,
    #[schemars(description = "Max results to return (default 20)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExportHandsParams {
    #[schemars(description = "Export format: 'csv' (default) or 'raw' (original hand history text)")]
    pub format: Option<String>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by villain name")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by result: won, lost, folded")]
    pub result: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Max hands to export (default 100)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetHandAsReplayerParams {
    #[schemars(description = "The hand ID to replay")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QuizHandParams {
    #[schemars(description = "Specific hand ID to quiz on (optional — picks a random qualifying hand if omitted)")]
    pub hand_id: Option<u64>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Target street for the quiz decision (default: auto-detect last hero decision street)")]
    pub street: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSimilarVillainsParams {
    #[schemars(description = "Target VPIP percentage (0-100)")]
    pub vpip: f64,
    #[schemars(description = "Target PFR percentage (0-100)")]
    pub pfr: f64,
    #[schemars(description = "Target 3-bet percentage (0-100, optional)")]
    pub three_bet: Option<f64>,
    #[schemars(description = "Target aggression factor (optional, typically 0-5)")]
    pub aggression_factor: Option<f64>,
    #[schemars(description = "Minimum hands to qualify (default 20)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Max results to return (default 5)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPreflopChartParams {
    #[schemars(description = "Hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: String,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReimportHandParams {
    #[schemars(description = "The hand ID to re-parse and re-embed")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetHandHistoryParams {
    #[schemars(description = "The hand ID to retrieve raw history text for")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CompareStatsParams {
    #[schemars(description = "First player name (defaults to hero)")]
    pub player_a: Option<String>,
    #[schemars(description = "Second player name (required)")]
    pub player_b: String,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CountHandsParams {
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by villain name")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by result: won, lost, folded")]
    pub result: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetShowdownHandsParams {
    #[schemars(description = "Villain name to find showdown hands for")]
    pub villain: String,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Max results to return (default 20)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetHandContextParams {
    #[schemars(description = "The hand ID to get surrounding context for")]
    pub hand_id: u64,
    #[schemars(description = "Number of hands before and after to include (default 5)")]
    pub window: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryHandsParams {
    #[schemars(description = "Raw SQL WHERE clause to filter hands (e.g. \"stakes = '$0.05/$0.10' AND hero_position = 'BTN'\"). Columns: id, game_type, variant, betting_limit, stakes, hero, hero_position, hero_cards, hero_result, board, pot_type, opponent_names, timestamp, is_bomb_pot, is_hi_lo, table_size, num_players, went_to_showdown, tournament_id, pot_amount")]
    pub filter: String,
    #[schemars(description = "Max results to return (default 50)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEquitySpotsParams {
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Max results to return (default 20)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetMultiwayStatsParams {
    #[schemars(description = "Minimum players seeing the flop (default 3)")]
    pub min_players: Option<u32>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetBankrollGraphParams {
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSqueezeSpotsParams {
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Max results to return (default 20)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetVillainProfileParams {
    #[schemars(description = "Villain name to profile")]
    pub villain: String,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Include pool averages alongside villain stats for context (default false). Shows how this villain compares to the average opponent.")]
    pub compare_to_pool: Option<bool>,
    #[schemars(description = "Minimum hands per player for pool stats (default 30). Only used when compare_to_pool is true.")]
    pub pool_min_hands: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPositionalMatchupsParams {
    #[schemars(description = "Villain name to analyze matchups against")]
    pub villain: String,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDatabaseHealthParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindLeaksParams {
    #[schemars(description = "Table size for baseline ranges: '6max' (default) or 'full_ring' (9-max)")]
    pub table_size: Option<String>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DetectTiltParams {
    #[schemars(description = "Minimum deviation from baseline to flag (percentage points, default 10). Lower values catch more subtle tilt.")]
    pub threshold: Option<f64>,
    #[schemars(description = "Minimum hands per session to analyze (default 20). Sessions with fewer hands are skipped.")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetStreetStatsParams {
    #[schemars(description = "Player name to analyze (defaults to hero)")]
    pub player: Option<String>,
    #[schemars(description = "Filter by villain name (only count hands where this villain was present)")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSizingProfileParams {
    #[schemars(description = "Player name to analyze (defaults to hero)")]
    pub player: Option<String>,
    #[schemars(description = "Filter by villain name (only count hands where this villain was present)")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPoolStatsParams {
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02'). Highly recommended — pool behavior varies drastically by stakes.")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Minimum hands a player must have to be included in the pool (default 30). Lower values include more players but noisier stats.")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetVillainTendenciesParams {
    #[schemars(description = "Villain name to analyze")]
    pub villain: String,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetBoardStatsParams {
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRangeAnalysisParams {
    #[schemars(description = "Hero position to analyze: BTN, CO, HJ, LJ, SB, BB")]
    pub position: String,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem (default). Only Hold'em hands have 2-card combos.")]
    pub variant: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTournamentSummaryParams {
    #[schemars(description = "Tournament ID to summarize")]
    pub tournament_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTournamentStackStatsParams {
    #[schemars(description = "Tournament ID to analyze stack trajectory for")]
    pub tournament_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPushFoldReviewParams {
    #[schemars(description = "Tournament ID (optional — reviews all tournaments if omitted)")]
    pub tournament_id: Option<u64>,
    #[schemars(description = "M-ratio threshold for push/fold territory (default 10)")]
    pub m_threshold: Option<f64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetBubblePlayParams {
    #[schemars(description = "Tournament ID to analyze bubble play for")]
    pub tournament_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEffectiveStacksParams {
    #[schemars(description = "Tournament ID (optional — analyzes all tournaments if omitted)")]
    pub tournament_id: Option<u64>,
    #[schemars(description = "Minimum pot size in big blinds to include (default 10)")]
    pub min_pot_bb: Option<f64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClusterVillainsParams {
    #[schemars(description = "Minimum hands to include a villain (default 20)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TagHandParams {
    #[schemars(description = "The hand ID to tag")]
    pub hand_id: u64,
    #[schemars(description = "Tags to add (e.g. 'bad call', 'review later', 'good bluff'). Provide one or more tags.")]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveTagParams {
    #[schemars(description = "The hand ID to remove tags from")]
    pub hand_id: u64,
    #[schemars(description = "Tags to remove. If empty, removes all tags from the hand.")]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTagsParams {
    #[schemars(description = "The hand ID to get tags for")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRunoutAnalysisParams {
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRunoutFrequenciesParams {
    #[schemars(description = "Which street to analyze: 'turn' (default) or 'river'")]
    pub street: Option<String>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTrendsParams {
    #[schemars(description = "Time bucket size: 'day', 'week' (default), or 'month'")]
    pub period: Option<String>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Filter to hands on or after this date (e.g. '2024-01-15')")]
    pub from_date: Option<String>,
    #[schemars(description = "Filter to hands on or before this date (e.g. '2024-02-15')")]
    pub to_date: Option<String>,
}
