mod action_encoder;
mod config;
mod parsers;
mod types;
mod summarizer;
mod embedder;
mod storage;
mod stats;
mod search;
mod sessions;
mod mcp;
mod scanner;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rmcp::ServiceExt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "pokervector", version, about = "Poker hand history engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Import hand histories from a directory (or all configured accounts)
    Import {
        /// Path to directory containing hand history files
        path: Option<PathBuf>,
        /// Hero player name (defaults to directory name)
        #[arg(long)]
        hero: Option<String>,
    },
    /// Show status (config + database info)
    Status,
    /// Start MCP server (stdio transport)
    Mcp {
        /// Hero player name (default: first configured account, or "Hero")
        #[arg(long)]
        hero: Option<String>,
    },
    /// Scan for installed poker clients and discover accounts
    Scan,
    /// Manually add an account
    AddAccount {
        /// Path to hand history directory
        path: PathBuf,
        /// Hero player name (defaults to directory name)
        #[arg(long)]
        hero: Option<String>,
        /// Poker site (default: acr)
        #[arg(long, default_value = "acr")]
        site: String,
    },
}

/// Import a single directory of hand histories.
async fn import_one(
    path: &Path,
    hero: &str,
    embedder: &mut embedder::Embedder,
    store: &storage::VectorStore,
) -> Result<(u64, u64, u64)> {
    println!("Importing from: {}", path.display());
    println!("Hero: {}", hero);
    println!();

    // Phase 1: Parse all hands
    let pattern = path.join("*.txt");
    let pattern_str = pattern.to_string_lossy();

    let mut all_hands: Vec<types::Hand> = Vec::new();
    let mut total_errors = 0u64;

    for entry in glob::glob(&pattern_str)? {
        let file_path = entry?;
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let content = std::fs::read_to_string(&file_path)?;
        let results = parsers::parse_auto(&content, hero);

        let mut file_hands = 0;
        let mut file_errors = 0;

        for result in results {
            match result {
                Ok(hand) => {
                    file_hands += 1;
                    all_hands.push(hand);
                }
                Err(e) => {
                    file_errors += 1;
                    eprintln!("  Error: {}", e);
                }
            }
        }

        println!("{}: {} hands, {} errors", filename, file_hands, file_errors);
        total_errors += file_errors;
    }

    println!();
    println!("Parsed {} hands ({} errors)", all_hands.len(), total_errors);

    if all_hands.is_empty() {
        println!("No hands to import.");
        return Ok((0, 0, total_errors));
    }

    // Phase 2: Summarize, embed, and store in batches
    let batch_size = 32;
    let mut imported = 0u64;
    let mut skipped = 0u64;
    let total = all_hands.len();

    for chunk in all_hands.chunks(batch_size) {
        let mut to_process: Vec<&types::Hand> = Vec::new();
        for hand in chunk {
            if store.hand_exists(hand.id).await? {
                skipped += 1;
            } else {
                to_process.push(hand);
            }
        }

        if to_process.is_empty() {
            continue;
        }

        let summaries: Vec<String> = to_process
            .iter()
            .map(|h| summarizer::summarize(h))
            .collect();
        let action_encodings: Vec<String> = to_process
            .iter()
            .map(|h| action_encoder::encode_action_sequence(h, hero))
            .collect();

        let summary_refs: Vec<&str> = summaries.iter().map(|s| s.as_str()).collect();
        let action_refs: Vec<&str> = action_encodings.iter().map(|s| s.as_str()).collect();

        let summary_embeddings = embedder.embed_batch(&summary_refs)?;
        let action_embeddings = embedder.embed_batch(&action_refs)?;

        let batch: Vec<(&types::Hand, &str, &str, storage::HandEmbeddings)> = to_process
            .into_iter()
            .zip(summaries.iter())
            .zip(action_encodings.iter())
            .zip(summary_embeddings.into_iter().zip(action_embeddings.into_iter()))
            .map(|(((hand, summary), action_enc), (sum_emb, act_emb))| {
                (hand, summary.as_str(), action_enc.as_str(), storage::HandEmbeddings {
                    summary: sum_emb,
                    action: act_emb,
                })
            })
            .collect();

        let batch_count = batch.len() as u64;
        store.upsert_hands_batch(batch).await?;
        imported += batch_count;

        print!(
            "\rImported {}/{} hands ({} skipped)...",
            imported, total, skipped
        );
    }

    println!(
        "\rImported {} hands, {} skipped, {} errors.     ",
        imported, skipped, total_errors
    );

    Ok((imported, skipped, total_errors))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Import { path, hero } => {
            let cfg = config::load_config()?;

            match path {
                Some(path) => {
                    // Explicit path: single import (same as before)
                    let hero = hero.unwrap_or_else(|| {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Hero")
                            .to_string()
                    });

                    println!("Loading embedding model...");
                    let mut embedder = embedder::Embedder::new()?;

                    println!("Opening database...");
                    let data_dir = config::data_dir();
                    let store = storage::VectorStore::new(data_dir.to_str().unwrap(), "poker_hands").await?;
                    store.ensure_table().await?;

                    import_one(&path, &hero, &mut embedder, &store).await?;
                }
                None => {
                    // No path: import all configured accounts
                    if cfg.accounts.is_empty() {
                        println!("No accounts configured. Run `pokervector scan` or `pokervector add-account <path>` first.");
                        return Ok(());
                    }

                    println!("Loading embedding model...");
                    let mut embedder = embedder::Embedder::new()?;

                    println!("Opening database...");
                    let data_dir = config::data_dir();
                    let store = storage::VectorStore::new(data_dir.to_str().unwrap(), "poker_hands").await?;
                    store.ensure_table().await?;

                    let mut total_imported = 0u64;
                    let mut total_skipped = 0u64;
                    let mut total_errors = 0u64;

                    for account in &cfg.accounts {
                        println!("=== {} ({}) ===", account.hero, account.site);
                        let (imported, skipped, errors) =
                            import_one(&account.path, &account.hero, &mut embedder, &store).await?;
                        total_imported += imported;
                        total_skipped += skipped;
                        total_errors += errors;
                        println!();
                    }

                    println!("Total: {} imported, {} skipped, {} errors",
                        total_imported, total_skipped, total_errors);
                }
            }
        }
        Commands::Status => {
            let cfg = config::load_config()?;
            let cfg_path = config::config_path();

            println!("Config: {}", cfg_path.display());
            if cfg.accounts.is_empty() {
                println!("Accounts: (none configured)");
            } else {
                println!("Accounts:");
                for account in &cfg.accounts {
                    let tag = if account.manual { "manual" } else { "scanned" };
                    println!("  {} ({}) — {} [{}]",
                        account.hero, account.site, account.path.display(), tag);
                }
            }
            let data_dir = config::data_dir();
            println!("Data: {}", data_dir.display());
            println!();

            match storage::VectorStore::new(data_dir.to_str().unwrap(), "poker_hands").await {
                Ok(store) => match store.count().await {
                    Ok(count) => println!("Stored hands: {}", count),
                    Err(e) => println!("Failed to read database: {}", e),
                },
                Err(e) => println!("Failed to open database: {}", e),
            }
        }
        Commands::Mcp { hero } => {
            let cfg = config::load_config()?;
            let hero = hero
                .or_else(|| cfg.accounts.first().map(|a| a.hero.clone()))
                .unwrap_or_else(|| "Hero".to_string());

            // MCP uses stdout for protocol messages; logging must go to stderr
            eprintln!("Loading embedding model...");
            let embedder = embedder::Embedder::new()?;

            eprintln!("Opening database...");
            let data_dir = config::data_dir();
            let store =
                storage::VectorStore::new(data_dir.to_str().unwrap(), "poker_hands").await?;

            eprintln!("Starting MCP server (hero: {})...", hero);
            let server = mcp::PokerVectorMcp::new(store, embedder, hero);
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
        }
        Commands::Scan => {
            let cfg = config::load_config()?;
            println!("Scanning for poker clients...");

            let scanned = scanner::scan_all();
            if scanned.is_empty() && cfg.accounts.is_empty() {
                println!("No poker clients found.");
                return Ok(());
            }

            let (merged, new_accounts) = config::merge_scanned(cfg, scanned);

            // Show all accounts
            if !merged.accounts.is_empty() {
                println!("Accounts:");
                for account in &merged.accounts {
                    let is_new = new_accounts.iter().any(|a| a.site == account.site && a.hero == account.hero);
                    let tag = if is_new {
                        "NEW"
                    } else if account.manual {
                        "manual"
                    } else {
                        "scanned"
                    };
                    println!("  {} ({}) — {} [{}]",
                        account.hero, account.site, account.path.display(), tag);
                }
            }

            if new_accounts.is_empty() {
                println!("No new accounts found.");
                return Ok(());
            }

            // Prompt to save
            print!("Save {} new account(s)? [Y/n] ", new_accounts.len());
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();

            if input.is_empty() || input == "y" || input == "yes" {
                config::save_config(&merged)?;
                println!("Saved to {}", config::config_path().display());
            } else {
                println!("Not saved.");
            }
        }
        Commands::AddAccount { path, hero, site } => {
            let mut cfg = config::load_config()?;

            let site_kind = match site.to_lowercase().as_str() {
                "acr" => config::SiteKind::Acr,
                other => {
                    eprintln!("Unknown site: {}. Supported: acr", other);
                    std::process::exit(1);
                }
            };

            let hero = hero.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Hero")
                    .to_string()
            });

            // Check for duplicates
            let exists = cfg.accounts.iter().any(|a| a.site == site_kind && a.hero == hero);
            if exists {
                println!("Account already exists: {} ({})", hero, site_kind);
                return Ok(());
            }

            let account = config::Account {
                site: site_kind,
                hero: hero.clone(),
                path: path.clone(),
                manual: true,
            };
            cfg.accounts.push(account);
            config::save_config(&cfg)?;

            println!("Added: {} ({}) — {}", hero, site_kind, path.display());
            println!("Saved to {}", config::config_path().display());
        }
    }

    Ok(())
}
