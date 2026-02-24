# PokerVector — MCP Ideas

## New Tools

### ~~`get_hand_history`~~ ✅
Return the original raw hand history text for a hand ID. Useful when the LLM needs to show the user the exact HH, or for copy-pasting into forums/solvers.

### ~~`compare_stats`~~ ✅
Takes two player names (or hero vs. a villain) and returns a side-by-side stat comparison. Right now the LLM has to call `get_stats` twice and diff them itself.

### `get_range_analysis`
Given filters (position, pot type, action taken), return the distribution of starting hands the hero/villain played. E.g. "what hands does PolarFox open from the CO?" Returns a frequency table grouped by hand category (pocket pairs, suited broadways, etc.).

### ~~`get_trends`~~ ✅
Winrate/stats over time (e.g. by week or by session). Lets the LLM answer "am I improving?" or "how has my 3-bet% changed over the last month?"

### ~~`find_leaks`~~ ✅
Automated leak detection. Compare hero's stats against baseline ranges (e.g. VPIP 22-28% for 6max is healthy, >35% is too loose). Return a list of potential leaks with the hands that demonstrate them.

### ~~`get_showdown_hands`~~ ✅
Return hands where a specific villain went to showdown, revealing their actual holdings. Gold for building villain reads.

### ~~`get_equity_spots`~~ ✅
Find hands where hero was all-in or facing a big decision, and return the board/holdings/pot odds. Feeds nicely into "was this a good call?" conversations.

### ~~`get_positional_matchups`~~ ✅
Hero vs. villain broken down by relative position (IP vs OOP). "How do I do against villain X when I'm out of position?"

### `get_bluff_candidates`
Find hands where hero folded but the action line suggests a bluff might have worked (e.g. villain bet small on scary boards, low aggression villain).

### `tag_hand`
Let the user tag/annotate hands with labels ("bad call", "good bluff", "review later") that persist in the DB. Then `search_hands` can filter by tag.

### ~~`get_street_stats`~~ ✅
Per-street aggression/fold frequencies. "How often does villain fold to turn barrels?" Goes deeper than the current aggregate stats.

## Tournament Tools

### `get_tournament_summary`
Results for a specific tournament: finish position, buy-in, payout, ROI, key hands. The session tools are cash-only right now.

### `get_tournament_stats`
Aggregate tournament stats: ROI%, ITM%, average finish, total buy-ins vs payouts, stats by buy-in level.

### `get_icm_spots`
Find tournament hands near the bubble or final table where stack sizes made the decision ICM-sensitive.

## Opponent Modeling

### ~~`get_villain_profile`~~ ✅
A single comprehensive tool that combines stats, showdown hands, sizing tendencies, and positional data into one villain report. Saves the LLM from orchestrating 4-5 tool calls.

### `get_villain_tendencies`
How a villain reacts to specific lines: "when hero c-bets and villain calls flop then faces a turn barrel, what does villain do?" Action-reaction sequences.

### `cluster_villains`
Group villains by play style using their stat profiles (nit, TAG, LAG, fish, maniac). Uses the action embeddings to find similarity.

## Multi-Hand Pattern Tools

### `get_sizing_profile`
Analyze bet sizing patterns for a player. "Villain bets 1/3 pot with draws and 2/3 with value" — return the distribution of bet sizes by street and outcome.

### `get_runout_analysis`
Given a set of hands (by filter), show how often hero wins on different board textures (monotone, paired, connected, etc.).

### ~~`detect_tilt`~~ ✅
Flag sessions or stretches where hero's play deviated from their baseline (VPIP spike, unusual aggression, etc.) after big losses.

## Quality of Life

### ~~`import_status`~~ ✅ (covered by `get_last_import`)
MCP tool to check how many hands are imported, last import time, any parse failures. So the LLM can tell the user "you haven't imported in 3 days."

### ~~`get_bankroll_graph`~~ ✅
Running profit/loss data points over time, suitable for the LLM to describe trends or for a client that renders charts.

### ~~`get_hand_context`~~ ✅
Given a hand ID, return the surrounding hands from the same table session (the 5 hands before and after). Useful for understanding table dynamics and momentum.

## Data Export / Interop

### ~~`export_hands`~~ ✅
Export filtered hands as PokerStars-format HH text, or as CSV. Useful for importing into solvers, trackers, or sharing on forums.

### ~~`get_hand_as_replayer`~~ ✅
Return hand data structured for a visual replayer (ordered actions with pot sizes at each step). Could feed into a web replayer down the line.

## Study Aids

### ~~`quiz_hand`~~ ✅
Return a hand stopped at a decision point (hide hero's action and the result). The LLM can use it to quiz the user: "What would you do here?"

### ~~`get_similar_villains`~~ ✅
"I'm about to play against someone with VPIP 45 / PFR 8 — which villains in my database play like that?" Find the closest stat match.

### ~~`get_preflop_chart`~~ ✅
Given hero's actual data, generate a grid showing open/fold/3bet frequencies for each starting hand combo from a given position. Shows what hero *actually does* vs. what they should do.

## Infrastructure

### ~~`reimport_hand`~~ ✅
Re-parse and re-embed a single hand (useful after parser bug fixes without re-importing everything).

### ~~`get_database_health`~~ ✅
Dedup count, hands missing embeddings, orphaned records, storage size. Maintenance diagnostics.

## Board Texture Analysis

### `get_board_stats`
How does hero perform on different board textures? Monotone, paired, connected, dry, wet. Breakdown by street and action taken.

### `get_runout_frequencies`
What turn/river cards show up most often after hero c-bets flop and gets called? Not about hero's play — about what boards the database actually contains.

## Multiway Analysis

### ~~`get_multiway_stats`~~ ✅
Stats filtered to 3+ players seeing the flop. Hero's multiway play often differs drastically from heads-up, and most stat tools don't distinguish.

### ~~`get_squeeze_spots`~~ ✅
Find hands where hero was in a squeeze-eligible position (raise + call in front). Show what hero actually did and the outcomes.

## Game Selection

### ~~`get_table_profitability`~~ ✅
Profit/loss broken down by table name or stakes level. "Which stakes am I most profitable at?"

### ~~`get_best_villains`~~ ✅
Flip of `list_villains` — rank opponents by how much hero has won from them. "Who are my most profitable opponents?"

### ~~`get_worst_villains`~~ ✅
The reverse. "Who am I losing the most to, and why?"

## Advanced Queries

### ~~`query_hands`~~ ✅
A power-user tool: pass a raw SQL WHERE clause against hand metadata. For queries too specific for the existing filters. The LLM can construct these.

### ~~`count_hands`~~ ✅
Simple filtered hand count without returning data. "How many 3-bet pots have I played from the SB?" Fast, lightweight.

## Notifications / Monitoring

### ~~`watch_directory`~~ ✅
Background file watcher that auto-imports new hand history files as they appear. Turns PokerVector into a live-updating system instead of manual imports.

### ~~`get_last_import`~~ ✅
When was the last import? How many new hands? Lightweight check so the LLM can prompt the user to re-import.

## Hand Categorization

### ~~`auto_tag_hands`~~ ✅
Automatically classify hands by archetype: "cooler", "suckout", "hero call", "big bluff", "set over set", "missed draw". Based on action patterns and outcomes.

### ~~`get_coolers`~~ ✅
Find hands where both players had strong holdings and a big pot resulted. Premium pair vs premium pair, set over set, etc.

## Enhancements to Existing Tools

### ~~Date range filters~~ ✅
Add `from_date` / `to_date` params to `search_hands`, `get_stats`, `list_villains`. Crucial for "how did I play last week?" type queries.

### ~~Villain positional breakdown~~ ✅
`get_stats` with `villain` filter could include positional breakdown for that matchup.

### ~~Pagination on `search_hands`~~ ✅
Add an `offset` param so the LLM can page through results instead of being capped at one batch.

## MCP Protocol Expansion

### Resources
Expose hero stats and session list as MCP resources (not just tools), so the client can pull them into context automatically without tool calls.

### Prompts
Predefined prompt templates like "Review my last session", "Analyze villain X", "Find my biggest leaks" that guide the LLM.
