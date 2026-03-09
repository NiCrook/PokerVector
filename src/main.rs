mod action_encoder;
mod config;
mod embedder;
mod importer;
mod mcp;
mod parsers;
mod scanner;
mod search;
mod sessions;
mod stats;
mod storage;
mod summarizer;
mod types;

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

/// Import a single directory with CLI progress output.
async fn import_one(
    path: &Path,
    hero: &str,
    embedder: &mut embedder::Embedder,
    store: &storage::VectorStore,
) -> Result<importer::ImportResult> {
    println!("Importing from: {}", path.display());
    println!("Hero: {}", hero);
    println!();

    let result = importer::import_directory(path, hero, embedder, store).await?;

    println!(
        "Imported {} hands, {} skipped, {} errors.",
        result.imported, result.skipped, result.errors
    );

    Ok(result)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Import { path, hero } => {
            let mut cfg = config::load_config()?;

            let mut total_imported = 0u64;
            let mut total_skipped = 0u64;
            let mut total_errors = 0u64;

            match path {
                Some(path) => {
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
                    let store =
                        storage::VectorStore::new(data_dir.to_str().unwrap(), "poker_hands")
                            .await?;

                    let result = import_one(&path, &hero, &mut embedder, &store).await?;
                    total_imported = result.imported;
                    total_skipped = result.skipped;
                    total_errors = result.errors;
                }
                None => {
                    if cfg.accounts.is_empty() {
                        println!("No accounts configured. Run `pokervector scan` or `pokervector add-account <path>` first.");
                        return Ok(());
                    }

                    println!("Loading embedding model...");
                    let mut embedder = embedder::Embedder::new()?;

                    println!("Opening database...");
                    let data_dir = config::data_dir();
                    let store =
                        storage::VectorStore::new(data_dir.to_str().unwrap(), "poker_hands")
                            .await?;

                    for account in &cfg.accounts {
                        println!("=== {} ({}) ===", account.hero, account.site);
                        let result =
                            import_one(&account.path, &account.hero, &mut embedder, &store).await?;
                        total_imported += result.imported;
                        total_skipped += result.skipped;
                        total_errors += result.errors;
                        println!();
                    }

                    println!(
                        "Total: {} imported, {} skipped, {} errors",
                        total_imported, total_skipped, total_errors
                    );
                }
            }

            // Save import log
            cfg.last_import = Some(config::ImportLog {
                timestamp: chrono::Utc::now().to_rfc3339(),
                hands_imported: total_imported,
                hands_skipped: total_skipped,
                errors: total_errors,
            });
            config::save_config(&cfg)?;
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
                    println!(
                        "  {} ({}) — {} [{}]",
                        account.hero,
                        account.site,
                        account.path.display(),
                        tag
                    );
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
            let server = mcp::PokerVectorMcp::new(store, embedder, hero, cfg.accounts);
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
                    let is_new = new_accounts
                        .iter()
                        .any(|a| a.site == account.site && a.hero == account.hero);
                    let tag = if is_new {
                        "NEW"
                    } else if account.manual {
                        "manual"
                    } else {
                        "scanned"
                    };
                    println!(
                        "  {} ({}) — {} [{}]",
                        account.hero,
                        account.site,
                        account.path.display(),
                        tag
                    );
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
            let exists = cfg
                .accounts
                .iter()
                .any(|a| a.site == site_kind && a.hero == hero);
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
