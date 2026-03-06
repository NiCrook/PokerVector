pub mod params;
mod analysis;
mod helpers;
mod prompts;
mod resources;
mod tools_advanced;
mod tools_export;
mod tools_hands;
mod tools_meta;
mod tools_search;
mod tools_sessions;
mod tools_stats;
mod tools_spots;
mod tools_tournament;
mod tools_villains;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router, ServerHandler,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config;
use crate::embedder::Embedder;
use crate::storage::VectorStore;

use params::*;

#[derive(Clone)]
pub struct PokerVectorMcp {
    store: Arc<VectorStore>,
    embedder: Arc<Mutex<Embedder>>,
    hero: String,
    accounts: Vec<config::Account>,
    tool_router: ToolRouter<Self>,
}

impl PokerVectorMcp {
    pub fn new(store: VectorStore, embedder: Embedder, hero: String, accounts: Vec<config::Account>) -> Self {
        Self {
            store: Arc::new(store),
            embedder: Arc::new(Mutex::new(embedder)),
            hero,
            accounts,
            tool_router: Self::tool_router(),
        }
    }
}

// Tool implementations

#[tool_router]
impl PokerVectorMcp {
    #[tool(description = "Search poker hand histories using natural language with optional filters. Returns matching hands ranked by relevance.")]
    async fn search_hands(
        &self,
        Parameters(params): Parameters<SearchHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_search_hands(params).await
    }

    #[tool(description = "Fetch full details of a specific hand by its numeric ID.")]
    async fn get_hand(
        &self,
        Parameters(params): Parameters<GetHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand(params).await
    }

    #[tool(description = "Get aggregate player statistics (VPIP, PFR, 3-bet%, etc.) with optional filters. Computes stats across all matching hands.")]
    async fn get_stats(
        &self,
        Parameters(params): Parameters<GetStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_stats(params).await
    }

    #[tool(description = "List tracked opponents with hand counts and key stats (VPIP, PFR, aggression, etc.).")]
    async fn list_villains(
        &self,
        Parameters(params): Parameters<ListVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_list_villains(params).await
    }

    #[tool(description = "List detected cash game sessions. Groups hands by table and play period. Sessions are separated by 30+ minutes of inactivity across all tables.")]
    async fn list_sessions(
        &self,
        Parameters(params): Parameters<ListSessionsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_list_sessions(params).await
    }

    #[tool(description = "Get a detailed review of a specific cash game session. Returns aggregate stats, per-table breakdown, and notable hands (biggest wins/losses).")]
    async fn review_session(
        &self,
        Parameters(params): Parameters<ReviewSessionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_review_session(params).await
    }

    #[tool(description = "Show profitability grouped by stakes or table. Returns hands played, net profit, and bb/100 for each group.")]
    async fn get_table_profitability(
        &self,
        Parameters(params): Parameters<TableProfitabilityParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_table_profitability(params).await
    }

    #[tool(description = "List villains hero profits the most against. Returns opponents sorted by hero's net profit descending.")]
    async fn get_best_villains(
        &self,
        Parameters(params): Parameters<BestVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_best_villains(params).await
    }

    #[tool(description = "List villains hero loses the most against. Returns opponents sorted by hero's net profit ascending (biggest losers first).")]
    async fn get_worst_villains(
        &self,
        Parameters(params): Parameters<WorstVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_worst_villains(params).await
    }

    #[tool(description = "Find hands with similar betting structure or narrative to a given hand ID. Default mode is 'action' (betting pattern similarity).")]
    async fn search_similar_hands(
        &self,
        Parameters(params): Parameters<SearchSimilarParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_search_similar_hands(params).await
    }

    #[tool(description = "Import new hand histories from configured account directories (or a specific path). Updates the database with any new hands found.")]
    async fn watch_directory(
        &self,
        Parameters(params): Parameters<WatchDirectoryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_watch_directory(params).await
    }

    #[tool(description = "Get information about the last import operation and total hands in the database.")]
    async fn get_last_import(
        &self,
        Parameters(params): Parameters<GetLastImportParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_last_import(params).await
    }

    #[tool(description = "Automatically classify hands into categories: cooler, hero_call, big_bluff, big_win, big_loss. Returns tagged hands grouped by category.")]
    async fn auto_tag_hands(
        &self,
        Parameters(params): Parameters<AutoTagHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_auto_tag_hands(params).await
    }

    #[tool(description = "Find cooler hands — showdown hands where hero invested heavily and lost. Sorted by pot size descending.")]
    async fn get_coolers(
        &self,
        Parameters(params): Parameters<GetCoolersParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_coolers(params).await
    }

    #[tool(description = "Export hands as CSV or raw hand history text with optional filters. CSV includes hand_id, timestamp, variant, stakes, hero position, cards, board, pot type, result, profit in BB, and pot size.")]
    async fn export_hands(
        &self,
        Parameters(params): Parameters<ExportHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_export_hands(params).await
    }

    #[tool(description = "Get a hand formatted as a step-by-step replay with running pot and stack sizes at each action.")]
    async fn get_hand_as_replayer(
        &self,
        Parameters(params): Parameters<GetHandAsReplayerParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand_as_replayer(params).await
    }

    #[tool(description = "Generate a quiz from a hand — shows the hand up to a decision point and hides hero's action + outcome. Great for studying decision-making. The answer is included separately.")]
    async fn quiz_hand(
        &self,
        Parameters(params): Parameters<QuizHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_quiz_hand(params).await
    }

    #[tool(description = "Classify all tracked opponents into play-style archetypes (Nit, TAG, LAG, Whale, Maniac, Rock) based on VPIP, PFR, and aggression factor. Groups villains into clusters with per-cluster averages.")]
    async fn cluster_villains(
        &self,
        Parameters(params): Parameters<ClusterVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_cluster_villains(params).await
    }

    #[tool(description = "Find villains in the database with similar stats to a target profile. Useful for finding players who play like a specific archetype (e.g. loose-aggressive, nit, etc.).")]
    async fn get_similar_villains(
        &self,
        Parameters(params): Parameters<GetSimilarVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_similar_villains(params).await
    }

    #[tool(description = "Build a preflop hand chart for hero at a given position. Shows open/3bet/call/fold/limp frequencies for each starting hand combo. Hold'em only.")]
    async fn get_preflop_chart(
        &self,
        Parameters(params): Parameters<GetPreflopChartParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_preflop_chart(params).await
    }

    #[tool(description = "Re-parse and re-embed a hand from its raw text. Useful after parser improvements to update a specific hand without full reimport.")]
    async fn reimport_hand(
        &self,
        Parameters(params): Parameters<ReimportHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_reimport_hand(params).await
    }

    #[tool(description = "Return the original raw hand history text for a hand ID. Useful for copy-pasting into forums, solvers, or review tools.")]
    async fn get_hand_history(
        &self,
        Parameters(params): Parameters<GetHandHistoryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand_history(params).await
    }

    #[tool(description = "Aggregate statistics across all opponents in the player pool. Shows distributions (mean, median, P25, P75) for key stats like VPIP, PFR, 3-bet%, c-bet, etc. Filter by stakes to see 'what does the average player do at this level?' Useful for contextualizing villain stats against the pool.")]
    async fn get_pool_stats(
        &self,
        Parameters(params): Parameters<GetPoolStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_pool_stats(params).await
    }

    #[tool(description = "Compare stats side-by-side for two players. Returns both stat profiles in a single response for easy comparison.")]
    async fn compare_stats(
        &self,
        Parameters(params): Parameters<CompareStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_compare_stats(params).await
    }

    #[tool(description = "Count hands matching filters without returning hand data. Fast, lightweight way to check how many hands match specific criteria.")]
    async fn count_hands(
        &self,
        Parameters(params): Parameters<CountHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_count_hands(params).await
    }

    #[tool(description = "Find hands where a specific villain went to showdown and revealed their holdings. Returns villain's cards, board, and outcome for each hand.")]
    async fn get_showdown_hands(
        &self,
        Parameters(params): Parameters<GetShowdownHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_showdown_hands(params).await
    }

    #[tool(description = "Get the surrounding hands from the same table as a given hand ID. Returns hands before and after for understanding table dynamics and momentum.")]
    async fn get_hand_context(
        &self,
        Parameters(params): Parameters<GetHandContextParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand_context(params).await
    }

    #[tool(description = "Power-user tool: query hands with a raw SQL WHERE clause against hand metadata columns. The LLM can construct arbitrary filters beyond what other tools support.")]
    async fn query_hands(
        &self,
        Parameters(params): Parameters<QueryHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_query_hands(params).await
    }

    #[tool(description = "Find hands where hero folded postflop but signals suggest a bluff/raise might have worked. Scores spots by: villain bet size (small = weak), heads-up, hero in position, river fold. Useful for studying missed aggression opportunities.")]
    async fn get_bluff_candidates(
        &self,
        Parameters(params): Parameters<GetBluffCandidatesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_bluff_candidates(params).await
    }

    #[tool(description = "Find hands where hero was all-in. Returns holdings, board, pot odds, and outcome for each all-in spot.")]
    async fn get_equity_spots(
        &self,
        Parameters(params): Parameters<GetEquitySpotsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_equity_spots(params).await
    }

    #[tool(description = "Get hero stats filtered to multiway pots (3+ players seeing the flop). Multiway play often differs drastically from heads-up.")]
    async fn get_multiway_stats(
        &self,
        Parameters(params): Parameters<GetMultiwayStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_multiway_stats(params).await
    }

    #[tool(description = "Get running profit/loss data points over time. Returns cumulative profit at each hand, suitable for graphing or trend analysis.")]
    async fn get_bankroll_graph(
        &self,
        Parameters(params): Parameters<GetBankrollGraphParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_bankroll_graph(params).await
    }

    #[tool(description = "Find hands where hero was in a squeeze-eligible spot (raise + cold call in front of hero preflop). Shows what hero did and the outcome.")]
    async fn get_squeeze_spots(
        &self,
        Parameters(params): Parameters<GetSqueezeSpotsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_squeeze_spots(params).await
    }

    #[tool(description = "Comprehensive villain report: stats, showdown hands, positional breakdown, and profit summary in one call. Saves orchestrating multiple tool calls.")]
    async fn get_villain_profile(
        &self,
        Parameters(params): Parameters<GetVillainProfileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_villain_profile(params).await
    }

    #[tool(description = "Hero vs villain broken down by hero position. Shows hands played and hero profit at each position to identify where hero has an edge or is exploited.")]
    async fn get_positional_matchups(
        &self,
        Parameters(params): Parameters<GetPositionalMatchupsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_positional_matchups(params).await
    }

    #[tool(description = "Automated leak detection. Compares hero's stats against healthy baseline ranges for 6-max or full-ring and flags potential leaks. Each leak includes the stat name, hero's value, the healthy range, a severity (minor/moderate/major), and an explanation.")]
    async fn find_leaks(
        &self,
        Parameters(params): Parameters<FindLeaksParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_find_leaks(params).await
    }

    #[tool(description = "Show the distribution of starting hands hero played from a given position. Groups by hand category (pocket pairs, suited broadways, offsuit broadways, suited connectors, etc.) and shows open/3bet/call/fold frequencies per combo. Hold'em only.")]
    async fn get_range_analysis(
        &self,
        Parameters(params): Parameters<GetRangeAnalysisParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_range_analysis(params).await
    }

    #[tool(description = "Analyze hero's performance on different board textures. Classifies flops by texture (monotone, two-tone, rainbow, paired, connected, high, low, dry, wet) and shows hands played, winrate, and c-bet frequency for each. Answers 'how do I perform on wet boards?' or 'should I c-bet more on dry flops?'")]
    async fn get_board_stats(
        &self,
        Parameters(params): Parameters<GetBoardStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_board_stats(params).await
    }

    #[tool(description = "Analyze how a villain reacts to specific betting lines. Shows action-reaction patterns: how villain responds to c-bets, barrels, checks, and probes on each street. Answers questions like 'when I c-bet flop and villain calls, what does villain do facing a turn barrel?'")]
    async fn get_villain_tendencies(
        &self,
        Parameters(params): Parameters<GetVillainTendenciesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_villain_tendencies(params).await
    }

    #[tool(description = "Analyze bet sizing patterns for a player by street. Shows distribution of bet/raise sizes as fractions of the pot (e.g. 25%, 33%, 50%, 66%, 75%, pot, overbet). Reveals sizing tells — e.g. 'villain bets small with draws and big with value'.")]
    async fn get_sizing_profile(
        &self,
        Parameters(params): Parameters<GetSizingProfileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_sizing_profile(params).await
    }

    #[tool(description = "Per-street action frequencies for a player. Shows bet/raise/call/check/fold counts and percentages on each street (flop, turn, river). Goes deeper than aggregate stats to answer 'how often does villain fold to turn barrels?' or 'what is hero's river aggression?'")]
    async fn get_street_stats(
        &self,
        Parameters(params): Parameters<GetStreetStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_street_stats(params).await
    }

    #[tool(description = "Detect sessions where hero's play deviated significantly from their baseline. Flags potential tilt — VPIP spikes, aggression changes, or unusual loss patterns after big hands. Compares per-session stats against overall averages.")]
    async fn detect_tilt(
        &self,
        Parameters(params): Parameters<DetectTiltParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_detect_tilt(params).await
    }

    #[tool(description = "Show how hero's stats change over time. Buckets hands by day, week, or month and computes key stats (VPIP, PFR, winrate, hands played, etc.) for each period. Useful for answering 'am I improving?' or 'how has my 3-bet% changed?'")]
    async fn get_trends(
        &self,
        Parameters(params): Parameters<GetTrendsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_trends(params).await
    }

    #[tool(description = "Get database health diagnostics: total hands, variant/stakes breakdowns, date range, data quality checks, and storage size.")]
    async fn get_database_health(
        &self,
        Parameters(params): Parameters<GetDatabaseHealthParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_database_health(params).await
    }

    #[tool(description = "Get a tournament overview: hand count, duration, starting/ending stack, blind levels, biggest wins/losses, and bustout detection.")]
    async fn get_tournament_summary(
        &self,
        Parameters(params): Parameters<GetTournamentSummaryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_tournament_summary(params).await
    }

    #[tool(description = "Track stack size and M-ratio across a tournament. Shows per-hand data points and summary stats (min/max/avg M, hands in push/fold territory).")]
    async fn get_tournament_stack_stats(
        &self,
        Parameters(params): Parameters<GetTournamentStackStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_tournament_stack_stats(params).await
    }

    #[tool(description = "Review hero's decisions with a low M-ratio. Flags questionable plays: folding late position with M < 6, limping with M < 8, non-shove raises with M < 5.")]
    async fn get_push_fold_review(
        &self,
        Parameters(params): Parameters<GetPushFoldReviewParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_push_fold_review(params).await
    }

    #[tool(description = "Analyze hero's bubble play in a tournament. Compares VPIP, steal%, and PFR between pre-bubble and bubble phases to detect over-tightening or fearless aggression.")]
    async fn get_bubble_play(
        &self,
        Parameters(params): Parameters<GetBubblePlayParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_bubble_play(params).await
    }

    #[tool(description = "Show effective stack depths for significant tournament pots. Returns hero stack, villain effective stack, and effective stack for each hand with pot >= min_pot_bb.")]
    async fn get_effective_stacks(
        &self,
        Parameters(params): Parameters<GetEffectiveStacksParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_effective_stacks(params).await
    }
}

#[tool_handler]
impl ServerHandler for PokerVectorMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "pokervector".to_string(),
                title: Some("PokerVector".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some("Poker hand history engine — search, stats, and analysis".to_string()),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "PokerVector: query your poker hand histories. Use search_hands for semantic or action-pattern search, \
                 get_hand for full hand details, get_stats for aggregate statistics, \
                 list_villains for opponent summaries, list_sessions to see cash game sessions, \
                 review_session for detailed session analysis, \
                 search_similar_hands to find structurally similar hands by ID, \
                 get_table_profitability to see profit by stakes or table, \
                 get_best_villains / get_worst_villains for opponent profitability, \
                 watch_directory to import new hand histories, \
                 get_last_import for import status, \
                 auto_tag_hands to classify hands by archetype (cooler, hero call, bluff, big win/loss), \
                 get_coolers to find showdown hands where hero invested heavily and lost, \
                 export_hands to export hands as CSV or raw text, \
                 get_hand_as_replayer for step-by-step hand replay with running pot/stacks, \
                 quiz_hand to generate a decision quiz from a hand, \
                 cluster_villains to classify all opponents into archetypes (Nit/TAG/LAG/Whale/Maniac/Rock), \
                 get_similar_villains to find opponents matching a stat profile, \
                 get_preflop_chart to build a preflop hand chart by position (Hold'em only), \
                 reimport_hand to re-parse and re-embed a specific hand, \
                 get_hand_history to retrieve raw hand history text, \
                 get_pool_stats for aggregate opponent pool statistics with distributions (mean/median/percentiles), \
                 compare_stats for side-by-side player stat comparison, \
                 count_hands for fast filtered hand counts, \
                 get_showdown_hands to see villain holdings at showdown, \
                 get_hand_context for surrounding table hands, \
                 query_hands for raw SQL WHERE filters, \
                 get_equity_spots for all-in hands with holdings and pot odds, \
                 get_multiway_stats for stats in multiway pots, \
                 get_bankroll_graph for cumulative profit over time, \
                 get_squeeze_spots for squeeze-eligible preflop situations, \
                 get_villain_profile for comprehensive single-villain reports, \
                 get_positional_matchups for hero vs villain by position, \
                 get_trends for stats over time (by day/week/month), \
                 find_leaks for automated leak detection against baseline ranges, \
                 detect_tilt to find sessions where play deviated from baseline, \
                 get_street_stats for per-street action frequencies (flop/turn/river), \
                 get_sizing_profile for bet sizing distribution analysis by street, \
                 get_villain_tendencies for action-reaction patterns against a specific villain, \
                 get_range_analysis for starting hand distribution by position, \
                 get_board_stats for hero performance by board texture (monotone/paired/wet/dry/etc.), \
                 get_bluff_candidates for missed bluff opportunities (postflop folds where a raise might have worked), \
                 get_database_health for database diagnostics, \
                 get_tournament_summary for tournament overviews, \
                 get_tournament_stack_stats for M-ratio/stack trajectory, \
                 get_push_fold_review for low-M decision review, \
                 get_bubble_play for bubble behavior analysis, \
                 and get_effective_stacks for tournament stack depth data. \
                 Also exposes resources (hero-stats, database-info) and prompts (review-last-session, analyze-villain, find-my-leaks)."
                    .to_string(),
            ),
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, ErrorData>> + Send + '_ {
        std::future::ready(Ok(ListResourcesResult {
            resources: self.list_resource_entries(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, ErrorData>> + Send + '_ {
        async move { self.read_resource_by_uri(&request.uri).await }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListPromptsResult, ErrorData>> + Send + '_ {
        std::future::ready(Ok(ListPromptsResult {
            prompts: self.list_prompt_entries(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResult, ErrorData>> + Send + '_ {
        std::future::ready(self.get_prompt_by_name(&request.name, &request.arguments))
    }
}
