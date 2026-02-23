# Desktop App Build Plan

Ship PokerVector as a local web application for Windows — single binary, browser-based UI, bundled local LLM, annual license key.

## Product Strategy

Two distribution channels, one codebase:

| User | Interface | LLM | Price |
|------|-----------|-----|-------|
| Power user | MCP server + Claude Code/Desktop | Their own Claude subscription | Free / open source |
| Everyone else | Desktop app (browser UI) | Bundled local model | $60-80/yr |
| Desktop + API key | Desktop app (browser UI) | Claude/OpenAI via API key | $60-80/yr (optional BYOK upgrade) |

The MCP server stays free and open source — builds community, credibility, and word-of-mouth. The desktop app is the paid product for the non-technical majority.

## Architecture Overview

```
┌─────────────────────────────────────────────┐
│  User's Browser (localhost:PORT)            │
│  ┌─────────────────────────────────────┐    │
│  │  React Frontend                     │    │
│  │  - Chat interface (LLM-powered)     │    │
│  │  - Stats dashboard                  │    │
│  │  - Hand viewer / session browser    │    │
│  │  - Settings (HH folder, LLM prefs) │    │
│  └──────────────┬──────────────────────┘    │
└─────────────────┼───────────────────────────┘
                  │ HTTP JSON API
┌─────────────────┼───────────────────────────┐
│  PokerVector Binary                         │
│  ┌──────────────┴──────────────────────┐    │
│  │  Axum HTTP Server                   │    │
│  │  - Serves static frontend files     │    │
│  │  - JSON API endpoints               │    │
│  │  - LLM router (local or API)        │    │
│  │  - License key validation           │    │
│  ├─────────────────────────────────────┤    │
│  │  LLM Layer                          │    │
│  │  - llama.cpp (bundled, default)     │    │
│  │  - Claude/OpenAI API (optional BYOK)│    │
│  ├─────────────────────────────────────┤    │
│  │  Core Engine (existing Rust code)   │    │
│  │  - Parser (ACR + future sites)      │    │
│  │  - Embedder (ONNX/BGE-small)       │    │
│  │  - LanceDB (embedded vectors)       │    │
│  │  - Search, Stats, Sessions          │    │
│  ├─────────────────────────────────────┤    │
│  │  System Tray (tray-icon crate)      │    │
│  │  - Open browser                     │    │
│  │  - Status indicator                 │    │
│  │  - Quit                             │    │
│  └─────────────────────────────────────┘    │
│  Data: ~/.pokervector/data/ (LanceDB)       │
│  Config: ~/.pokervector/config.toml         │
│  License: ~/.pokervector/license.key        │
│  Model: ~/.pokervector/models/ (GGUF)       │
└─────────────────────────────────────────────┘
```

## Components

### 1. License Module

**Purpose:** Enforce annual subscription. Offline validation, no server needed.

**How it works:**
- You (the developer) have a private Ed25519 key, kept secret
- When a user pays, you generate a license key: `base64(json_payload + signature)`
- Payload: `{ "email": "user@email.com", "expires": "2027-02-23", "edition": "standard" }`
- App has the public key baked in, verifies the signature + checks expiry date
- Invalid or expired key → app shows "enter license key" screen, blocks all other functionality

**Key generation (your side):**
- Simple CLI tool or script that takes email + duration, outputs a license key
- Run locally, never deployed anywhere

**Crates:** `ed25519-dalek` for signing/verification, `base64` for encoding.

**Security:** Not uncrackable (someone could patch the binary), but sufficient. Poker tool market isn't targeted by pirates — the audience is small and willing to pay.

### 2. LLM Layer

**Purpose:** Power the chat interface. Two backends, one interface.

#### Default: Bundled Local Model (llama.cpp)

- `llama-cpp-rs` (Rust bindings to llama.cpp) compiled into the binary
- No separate install, no Ollama, no Docker — just works
- Model file (GGUF format) ships with installer or downloads on first run

**Key context: This is a post-session study tool, not a live HUD.** When users run PokerVector, they're not playing — full system resources (CPU, RAM, GPU) are available for inference. This means we can run larger, higher-quality models than a typical "background app."

**Model selection (tiered by system RAM):**

| Model | Size (Q4) | RAM Needed | Quality | Use Case |
|-------|-----------|------------|---------|----------|
| Qwen3-14B | ~8 GB | 12 GB | Good | Budget/older hardware |
| Qwen3-30B-A3B (MoE) | ~17 GB | 20 GB | Great | **Default recommendation.** MoE = only 3.3B params active per token, so it's fast like a small model but smart like a big one. Best balance of speed and quality. |
| DeepSeek R1 32B | ~18 GB | 24 GB | Near-cloud | Best local reasoning. Built-in chain-of-thought. Closest to cloud API quality for analytical tasks. |

**Recommendation:** Default to **Qwen3-30B-A3B** — it delivers ~90% of flagship model quality at fast inference speeds thanks to MoE architecture. Detect available RAM at startup and recommend the best model the user's hardware can support. User can override in settings.

**Inference config:**
- Context window: 8192-16384 tokens (study tool can afford larger context — more hands/stats in RAG)
- Threads: auto-detect CPU cores (all available, not competing with poker client)
- GPU: CUDA/Vulkan acceleration when available (llama.cpp supports this). Most gaming PCs (common among poker players) have capable GPUs — leverage them for faster inference.
- Streaming: token-by-token via SSE to frontend

**Poker-specific optimization:**
- Heavy system prompt with poker terminology, position names, bet sizing concepts
- RAG context: inject relevant hands, villain stats, session data before the user's question
- Larger context window means we can feed more data per query — multiple hand histories, full villain profiles, session-level trends
- The model doesn't need to be a poker expert — it needs to summarize and pattern-match data we feed it
- Fine-tuning later (with poker training data) would be a major differentiator

#### Optional: Cloud API (BYOK)

- User pastes Claude or OpenAI API key in settings
- App routes chat requests to the cloud API instead of local model
- Significantly better analysis quality (Claude Opus >> any 7B model)
- Settings toggle: "Local Model" vs "Cloud API"
- API key stored encrypted in config (or OS keychain)

**The LLM router:**
```
User message → Backend receives
  → Retrieve relevant context from LanceDB (hands, stats, villain data)
  → Build prompt (system prompt + context + user message)
  → Route to:
      Local: llama.cpp inference
      Cloud: reqwest → Claude/OpenAI API
  → Stream response back to frontend via SSE
```

**Crates:** `llama-cpp-rs` (local inference), `reqwest` (API calls), `tokio` (async streaming).

### 3. Backend (Axum HTTP Server)

**Purpose:** Serve the frontend, expose JSON API, manage LLM routing.

**API Endpoints:**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Serve frontend (static files) |
| `/api/health` | GET | Server status + model status |
| `/api/license` | POST | Submit/validate license key |
| `/api/settings` | GET/PUT | HH folder path, LLM preferences, API key |
| `/api/import` | POST | Trigger hand history import |
| `/api/import/status` | GET | Import progress (SSE or polling) |
| `/api/hands/search` | POST | Search hands (semantic or action) |
| `/api/hands/:id` | GET | Get single hand details |
| `/api/stats` | GET | Aggregate stats with filters |
| `/api/villains` | GET | List villains with stats |
| `/api/sessions` | GET | List detected sessions |
| `/api/sessions/:id` | GET | Session review |
| `/api/similar/:id` | GET | Find similar hands |
| `/api/chat` | POST | Send message → LLM router → SSE response |
| `/api/chat/history` | GET | Retrieve conversation history |
| `/api/model/status` | GET | Local model loaded? RAM usage? |

**Chat endpoint detail (`/api/chat`):**
1. Frontend sends `{ message, conversation_id, context_mode }`
2. Backend analyzes the message to determine what data to retrieve
3. Queries LanceDB: relevant hands, villain stats, session data
4. Builds prompt: system prompt + retrieved context + conversation history + user message
5. Routes to local model or cloud API based on settings
6. Streams response back via SSE (Server-Sent Events)

**Crates:** `axum`, `tower-http` (static files, CORS), `reqwest` (cloud API calls), `tokio`, `serde_json`.

### 4. Frontend (Web UI)

**Purpose:** The product the user sees and interacts with.

**Tech:** React + TypeScript, bundled as static files served by Axum.

**Pages/Views:**

1. **License Entry** — first-run screen, paste license key
2. **Setup Wizard** — select HH folder, choose LLM (local vs API key), trigger first import
3. **Dashboard** — overview stats (hands imported, VPIP, PFR, winrate chart)
4. **Chat** — main interface, ask questions about your game, get analysis
5. **Hand Browser** — search/filter hands, view individual hand histories
6. **Villain Profiles** — per-villain stats and tendencies
7. **Sessions** — session list, session review with notable hands
8. **Settings** — HH folder, LLM config, license info, auto-import toggle

**The chat interface is the core product.** Everything else (stats, hands, sessions) supports it. Users ask natural language questions, the backend retrieves relevant data, the LLM provides analysis.

**Chat UX considerations:**
- Show "thinking" indicator while model generates
- Display the context used (which hands/stats were retrieved) in a collapsible panel
- Suggested follow-up questions ("Ask about this villain's 3-bet range")
- One-click actions from chat ("Show me the hand" → navigates to hand viewer)

**Build:** Vite for dev/build. Output goes to a `frontend/dist/` directory, embedded in the Rust binary at compile time via `rust-embed`.

### 5. System Tray

**Purpose:** App runs in background, accessible from system tray.

**Behavior:**
- App starts → tray icon appears → browser opens to `localhost:PORT`
- Tray menu: "Open PokerVector" (opens browser), "Status", "Quit"
- Closing the browser tab doesn't kill the server
- "Quit" from tray stops the server

**Crate:** `tray-icon` + `winit` (Windows-native tray support).

### 6. Auto-Import (Background)

**Purpose:** Watch the HH folder for new files, import automatically.

- File watcher on the configured HH directory (`notify` crate)
- When new `.txt` files appear, parse + embed + store
- Frontend shows notification: "12 new hands imported"
- Runs on a background Tokio task

### 7. Installer / Packaging

**Purpose:** One-click install for Windows users.

**What gets installed:**
| Component | Size | Location |
|-----------|------|----------|
| PokerVector binary | ~15-20 MB | `C:\Program Files\PokerVector\` |
| ONNX embedding model (BGE-small) | ~45 MB | Bundled in binary or alongside |
| GGUF LLM model (Qwen3-30B-A3B Q4) | ~17 GB | `~/.pokervector/models/` |
| Frontend assets | ~2-5 MB | Embedded in binary |

**Installer approach:** NSIS or Inno Setup (both free, mature, Windows-native).

**What the installer does:**
1. Copies binary + embedding model to `C:\Program Files\PokerVector\`
2. Creates start menu shortcut
3. Optionally sets auto-start on login
4. Creates `~/.pokervector/` directory structure
5. Downloads LLM model on first run (or bundles in a "full" installer variant)

**Two installer variants:**
- **Lite (~70 MB):** Binary + embedding model. Downloads LLM model on first launch (setup wizard lets user choose model based on their RAM).
- **Full (~17 GB):** Everything bundled with Qwen3-30B-A3B. No internet needed after install.

### 8. Distribution Website

**Purpose:** Marketing, payment, download, license delivery.

**Minimal viable site:**
- Landing page (what it does, screenshots, price)
- Buy button (Stripe Checkout or LemonSqueezy)
- After payment: show license key + download link (lite + full options)
- Auto-email the license key as backup

**Hosting:** Static site (Vercel, Netlify) + Stripe for payments. Near zero cost.

**License generation flow:**
- Stripe webhook fires on successful payment
- Serverless function (Cloudflare Worker / Vercel function) generates license key
- Stores in a simple DB (Supabase free tier) for reissue if user loses key
- Returns key to the thank-you page + sends email

## Build Order

### Phase 1: Foundation
1. Migrate storage to LanceDB (see `lancedb-migration.md`)
2. Build license key generation + validation module
3. Verify core pipeline works end-to-end without Qdrant

### Phase 2: LLM Integration
4. Integrate llama.cpp via `llama-cpp-rs`
5. Build the LLM router (local model + cloud API abstraction)
6. Design poker-specific system prompts and RAG context building
7. Test inference quality with poker questions

### Phase 3: HTTP API
8. Add Axum server with JSON API endpoints
9. Wire up existing search/stats/session logic to API routes
10. Add chat endpoint with SSE streaming
11. Add import endpoint with progress reporting

### Phase 4: Frontend
12. Scaffold React app with Vite
13. Build license entry + setup wizard
14. Build chat interface (the core product)
15. Build stats dashboard, hand browser, villain profiles, sessions

### Phase 5: Desktop Polish
16. Add system tray support
17. Add file watcher for auto-import
18. Embed frontend assets in binary (`rust-embed`)
19. Bundle models in release build

### Phase 6: Package & Ship
20. Create Windows installer (NSIS or Inno Setup)
21. Build distribution website
22. Set up Stripe + license generation
23. Beta test with real users

## Pricing Model

**Annual license: $60-80/yr**

Comparable tools:
- PokerTracker 4: $100 one-time (+ upgrades)
- Hold'em Manager 3: $100 one-time
- GTO Wizard: $50-250/mo
- PokerVector differentiator: AI-powered analysis vs just stats, no ongoing LLM cost for the user

The bundled local model means **zero ongoing cost** for the user. No API fees, no cloud subscription. The $60-80/yr license is the only cost. This is a strong selling point vs tools that would require a $20/mo API key.

**Positioning:** This is a post-session study tool, not a live HUD. Users run it after their session to review hands, analyze villains, and find leaks. The poker client is closed, so full system resources (CPU, RAM, GPU) are available for high-quality local inference. This is the same workflow as reviewing hand histories in PokerTracker — but with AI that can answer questions about your play.

## Risks & Considerations

1. **Local model quality** — Even 30B models won't match Claude Opus for nuanced poker reasoning. Mitigate with strong RAG context (feed the model lots of data) and poker-specific prompting. Users who want cloud-quality can add a Claude API key.

2. **RAM requirements** — Qwen3-30B-A3B Q4 needs ~20GB RAM. Most gaming PCs (16-32GB) can handle it. Since this is a post-session study tool (poker client is closed), full RAM is available. Offer Qwen3-14B (~12GB) as fallback for 16GB machines. Setup wizard detects RAM and recommends.

3. **First-run model download** — 17GB download on first launch needs good UX. Show clear progress bar with speed/ETA. Offer full installer as alternative. Support resume-on-interrupt.

4. **llama.cpp build complexity** — `llama-cpp-rs` depends on C++ compilation. May need to vendor/prebuild for Windows. Test on clean Windows installs.

5. **GPU acceleration** — llama.cpp supports CUDA/Vulkan. Most poker players have gaming PCs with decent GPUs. Ship with CUDA support from the start — the speed difference is dramatic (5-10x faster inference). Vulkan as fallback for AMD GPUs.

6. **Binary size** — Rust binary + ONNX model + llama.cpp + frontend assets could be large. Keep under 100MB for the lite installer.

## Open Questions

1. **Qwen3-30B-A3B vs DeepSeek R1 32B as default?** Both strong. Qwen3 MoE is faster (less active params), DeepSeek R1 has better chain-of-thought reasoning. Need to test both with poker-specific prompts and compare quality/speed tradeoff.
2. **Model download UX** — Lite installer + first-run download vs full installer vs both? Leaning both.
3. **Conversation persistence** — Store chat history in LanceDB or separate SQLite? Probably SQLite for simplicity.
4. **Frontend framework** — React (larger ecosystem) vs Svelte (lighter, faster solo dev). Leaning React.
5. **Auto-update mechanism** — `self_update` crate to check for new versions. Nice to have, not MVP.
6. **Mac support timeline** — Windows-only first. LanceDB, Axum, llama.cpp are all cross-platform so Mac port is feasible later.
7. **Fine-tuning** — Could fine-tune a model on poker training data for much better analysis. Big lift but massive differentiator. Post-launch.
8. **CUDA bundling** — Ship CUDA runtime with installer or require user to have it? Most gamers have NVIDIA drivers installed already. Could detect and enable automatically.
