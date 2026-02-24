# Stats Expansion Plan

Candidate stats to add to `PlayerStats` beyond the current 33 metrics. Grouped by priority.

## Current Coverage

VPIP, PFR, 3-bet%, fold-to-3bet, 4-bet%, fold-to-4bet, AF, winrate bb/100, WTSD%, W$SD%, c-bet flop/turn/river, fold-to-cbet flop/turn/river, steal%, fold-to-steal BB/SB, limp%/call/fold/raise, donk bet%, float%, check-raise%, probe bet%, squeeze%, cold call%, WWSF, overbet%, flop seen%, avg pot size, showdown/non-showdown winnings, positional stats (VPIP/PFR/3-bet/cbet/winrate per position).

## High Priority

### Aggression Frequency (AFq%)
- `(Bet + Raise) / (Bet + Raise + Call + Check + Fold)` postflop
- More stable than AF with small samples since passive actions are in the denominator
- AF can be infinite (no calls); AFq% is always 0-100
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Small — just count checks and folds postflop alongside existing bet/raise/call counters

### Raise C-bet Flop / Turn
- How often player *raises* when facing a c-bet (the third option beyond fold/call)
- Currently we track fold-to-cbet but not raise-cbet
- Exploitable if very low (never fights back) or very high (spew)
- **Files:** `cbet.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — extend `CbetResult` with `raise_cbet_flop: (bool, bool)` and `raise_cbet_turn: (bool, bool)`, detect raise response after PFA c-bet

### Check-Raise by Street
- Break existing aggregate `check_raise_pct` into `check_raise_flop`, `check_raise_turn`, `check_raise_river`
- Street-level CR% is much more actionable than aggregate
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — `check_raise_analysis` currently returns aggregate counts across streets; split into per-street tracking

### Won Hand % (WHHP)
- `hands_won / hands_played` — simple win rate by count
- Quick read on overall results independent of dollar amounts
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Small — count hands where `hero_collected > hero_invested`

## Medium Priority

### Fold to Donk Bet
- How often the PFA folds when donked into on the flop
- Pairs with existing `donk_bet_pct` to find exploitable spots
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — need to identify PFA facing a donk bet and track fold response

### Missed C-bet Then Fold
- PFA checks flop (no c-bet), then folds to a bet
- Reveals who gives up completely when they don't c-bet
- **Files:** `cbet.rs` or new analysis fn, `calculate.rs`, `mod.rs`
- **Effort:** Medium — detect PFA who checked flop, then track fold to subsequent bet

### C-bet IP vs OOP
- Split flop c-bet frequency by whether PFA is in position or out of position
- Most players c-bet significantly more IP; seeing the split exposes OOP leaks
- **Files:** `cbet.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — use existing `is_player_ip` helper to classify c-bet opportunities

### Average Winning / Losing Hand Size
- Avg $ won when winning, avg $ lost when losing
- Reveals whether someone wins small pots and loses big ones (or vice versa)
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Small — track sum and count of positive/negative hand results separately

### Delayed C-bet
- PFA checks flop, then bets turn (delayed continuation bet)
- Common line in 3-bet pots and on wet boards; reveals strategic sophistication
- **Files:** `cbet.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — detect PFA who checked flop (no c-bet) then bet turn as first aggressor

### Bet/Fold %
- Bet then fold to a raise on the same street
- Detects weak bets with no backup — highly exploitable if high
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — track sequences where player bets then folds to a raise within the same street

### Check-Call %
- Check then call a bet postflop (aggregate across streets)
- The passive postflop line — high values indicate a calling station tendency
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — detect check followed by call from the same player on the same street

### Check-Fold %
- Check then fold to a bet postflop (aggregate across streets)
- High values mean player gives up too easily when they don't lead
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — detect check followed by fold from the same player on the same street

### Fold to River Bet
- How often player folds to any river bet (broader than fold-to-cbet-river which requires PFA)
- Key exploitative stat — if high, bluff rivers more
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — detect player facing a river bet and tracking fold response

### AF by Street
- Aggression factor broken down into flop AF, turn AF, river AF
- Reveals where a player is aggressive vs passive — many players barrel flop but shut down on turn
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Small — split existing postflop bet/raise/call counters by street

### C-bet Sizing
- Average c-bet size as fraction of pot on flop/turn/river
- Sizing tells: small c-bets may indicate range bets, large ones polarized
- **Files:** `cbet.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — capture bet amount and pot size at time of c-bet, use `estimate_pot_at_street`

### 3-bet vs Steal
- 3-bet frequency specifically against late position opens (CO/BTN/SB)
- Different from general 3-bet% — many players only 3-bet vs steals
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — combine steal detection with 3-bet analysis to identify overlap

### Blind Defense %
- How often BB defends vs a raise (calls or raises instead of folding)
- Effectively `1 - fold_to_open_from_BB`
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — track BB facing a raise and whether they fold

### Isolation Raise %
- Raising after one or more limpers to isolate a weak player
- Separate from open-raise; indicates exploitative tendencies
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — detect limp(s) before player's raise action

### Overlimp %
- Limping behind existing limpers instead of raising
- High values indicate a passive/weak preflop strategy
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — detect call of BB after one or more other limps with no raise

### Preflop Raise Sizing
- Average open raise size in BB
- Sizing tells: 2x vs 3x vs 4x openers play very differently
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — capture raise `to` amount and divide by BB for open raises

### All-in Preflop %
- Frequency of getting all-in before the flop
- High values may indicate shove-or-fold style or tilt
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — detect any preflop action with `all_in: true`

### Fold to Squeeze
- How often player folds when facing a squeeze (3-bet after calling an open)
- Complements existing `squeeze_pct` from the other side
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — detect player called an open, then someone 3-bets behind, track fold response

### 3-bet Sizing
- Average 3-bet size in BB
- Sizing tells: small 3-bets indicate merged range, large ones polarized
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Small — capture raise `to` amount and divide by BB when raise is a 3-bet

### Check-Raise Follow-through
- CR flop then bets turn — shows commitment vs one-and-done bluffs
- Low follow-through = often bluffing the CR; high = strong or committed
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — detect CR on flop, then check for bet on turn from same player

### Fold to Float Bet
- PFA c-bets flop, gets called, checks turn, faces bet, folds
- The "give up to float" line — high values mean IP callers can profitably float
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — chain of: PFA c-bet flop → called → PFA checks turn → faces bet → folds

### Fold to Probe Bet
- How often player folds when facing a probe bet (checked flop, opponent bets turn)
- Complements existing `probe_bet_pct` from the defender's perspective
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — detect PFA who checked flop, faces turn bet, track fold

### Probe Bet by Street
- Split aggregate `probe_bet_pct` into turn and river separately
- Turn probes vs river probes have different strategic implications
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — `probe_analysis` currently aggregates; split per-street

### Postflop Bet Sizing
- Average bet size as % of pot (aggregate across streets)
- General sizing tendency — small ball vs big bet players
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — capture bet amounts and pot sizes using `estimate_pot_at_street`

### River Bet Sizing
- Average river bet as % of pot
- River sizing is the most telling street for sizing tells
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — capture river bet amount and pot at river start

### Overbet by Street
- Split aggregate `overbet_pct` into flop/turn/river
- River overbets have very different meaning than flop overbets
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — `overbet_analysis` currently aggregates; split per-street

### 3-bet Pot C-bet %
- C-bet frequency specifically in 3-bet pots
- Often very different from SRP c-bet — many players c-bet 3-bet pots at near 100%
- **Files:** `cbet.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — filter c-bet opportunities by pot type using `classify_pot_type`

### Win Rate by Pot Type
- Winrate (bb/100) in SRP vs 3-bet vs 4-bet pots
- Uses existing `classify_pot_type` — reveals where profit/losses come from
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Medium — maintain separate net_won/hands accumulators keyed by pot type

### River Call Efficiency
- % of river calls that are winning calls
- Reveals if someone is a crying call station or a good bluff catcher
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Medium — detect river call actions from hero, check if hero won the hand

## Low Priority

### Raise Flop Bet (non-cbet)
- How often player raises any flop bet, not just c-bets
- General flop aggression measure
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`

### Cold 4-bet %
- 4-betting without having previously entered the pot (different from standard 4-bet which requires opening first)
- Indicates very strong range
- **Files:** `preflop.rs`, `calculate.rs`, `mod.rs`

### Bet When Checked To (River)
- River stab frequency when given the opportunity
- Exploitable if very low (never bluffs river) or very high
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`

### Saw Turn / Saw River %
- Like `flop_seen_pct` but for later streets
- Shows how deep into hands a player typically goes
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Small — reuse `hero_saw_street` helper with Turn/River

### Donk Bet by Street
- Split existing aggregate `donk_bet_pct` into flop/turn/river
- Different streets have different donk bet implications
- **Files:** `postflop.rs`, `calculate.rs`, `mod.rs`
- **Effort:** Medium — `donk_bet_analysis` currently aggregates; split per-street

### Multiway vs Heads-up Splits
- Key stats (VPIP, c-bet, WTSD, AF) split by whether pot was multiway or heads-up
- Players behave very differently multiway — c-bet% drops significantly
- **Files:** `calculate.rs`, `mod.rs`
- **Effort:** Large — need to determine pot player count at each street and maintain parallel stat accumulators
