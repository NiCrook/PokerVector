# Roadmap

Future directions for PokerVector beyond the completed MVP.

## Richer Analysis

- **Session tracking** — group hands by session, track win/loss over time, session duration, tilt detection
- **Positional stats** — break down VPIP/PFR/3-bet by position (BTN, CO, etc.)
- **Villain profiling** — per-villain stat summaries, tendencies, leak detection
- **Bankroll tracking** — profit/loss curves, stakes progression
- **Showdown analysis** — what hands do opponents show down with, by position and action line

## New MCP Tools

- **`get_session_summary`** — recap a session with key hands and P&L
- **`compare_players`** — side-by-side stat comparison
- **`find_leaks`** — flag statistical anomalies (e.g., fold-to-3bet too high, c-bet too low)
- **`hand_range_analysis`** — what ranges is a villain showing up with in specific spots

## Advanced Embedding / Search

- **Action-sequence embeddings** — embed the betting line itself (not just the natural language summary) for finding structurally similar hands
- **Cluster analysis** — group similar hands to find patterns you didn't think to search for
- **"Hands like this"** — given a hand, find the most similar hands you've played before
