use std::path::PathBuf;

use crate::config::{Account, SiteKind};

/// Scan for ACR hand history directories.
/// Checks `C:\AmericasCardroom\handHistory\` for subdirectories containing `.txt` files.
fn scan_acr() -> Vec<Account> {
    let base = PathBuf::from(r"C:\AmericasCardroom\handHistory");
    if !base.is_dir() {
        return Vec::new();
    }

    let mut accounts = Vec::new();
    let entries = match std::fs::read_dir(&base) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Check if this subdirectory contains any .txt files
        let has_txt = match std::fs::read_dir(&path) {
            Ok(files) => files.flatten().any(|f| {
                f.path()
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("txt"))
                    .unwrap_or(false)
            }),
            Err(_) => false,
        };
        if has_txt {
            let hero = entry.file_name().to_string_lossy().to_string();
            accounts.push(Account {
                site: SiteKind::Acr,
                hero,
                path,
                manual: false,
            });
        }
    }

    accounts
}

/// Run all registered site scanners and return combined results.
pub fn scan_all() -> Vec<Account> {
    let mut results = Vec::new();
    results.extend(scan_acr());
    // Future: results.extend(scan_pokerstars());
    results
}
