//! Phase 1: Wildcard Pre-Check Detection.
//!
//! Before validating real email addresses, this module probes each unique domain
//! with a random-looking fake address. If the SMTP server accepts the fake address,
//! the domain is flagged as a catch-all / wildcard domain.
//!
//! Only runs when the method is [`Method::Smtp`] and
//! wildcard detection is not disabled via `-d`.

use std::collections::HashSet;
use colored::*;
use check_if_email_exists::{check_email, CheckEmailInputBuilder, Reachable};
use crate::Method;

/// Run wildcard pre-check on all unique domains.
///
/// For each domain, sends a fake email address through the SMTP pipeline.
/// If the server reports the address as reachable, the domain is added
/// to the returned set.
///
/// # Parameters
///
/// * `method` — Validation method (only SMTP triggers precheck)
/// * `disable_wildcard` — Skip wildcard detection if true
/// * `verbose` — Print per-domain progress to STDERR
/// * `unique_emails` — Deduplicated list from Phase 0
/// * `is_quiet` — Suppress STDERR output if true
///
/// # Returns
///
/// A `HashSet<String>` of domains that accept any recipient (catch-all behavior).
/// Empty if method is not SMTP or wildcard detection is disabled.
pub async fn run(method: Method, disable_wildcard: bool, verbose: bool, unique_emails: &[String], is_quiet: bool, _concurrency: Option<usize>) -> HashSet<String> {
    let mut wildcard_domains = HashSet::new();

    if method != Method::Smtp || disable_wildcard {
        return wildcard_domains;
    }

    let mut unique_domains = HashSet::new();
    for email in unique_emails {
        if let Some(domain) = email.split('@').nth(1) {
            unique_domains.insert(domain.to_string());
        }
    }

    if !is_quiet { eprintln!("{}", "==> Phase 1: Running Wildcard Pre-Check Detection...".cyan()); }
    
    for domain in unique_domains {
        if verbose { eprint!("[*] Testing wildcard setup for domain: {}\r", domain); }

        let test_email = format!("dlfAxs7TGR91OhmWCbDiqtpcwEEARRJf@{}", domain);
        let check_input = CheckEmailInputBuilder::default().to_email(test_email).build().unwrap();
        let result = check_email(&check_input).await;

        match result.is_reachable {
            Reachable::Safe | Reachable::Risky => {
                if !is_quiet { eprintln!("{}", format!("[!] Wildcard/Catch-All on domain '{}' detected!", domain).red()); }
                wildcard_domains.insert(domain);
            }
            _ => {
                if verbose { eprint!("[+] Domain '{}' handles missing accounts correctly.\r", domain); }
            }
        }
    }
    
    if !is_quiet { eprintln!("{}", "--------------------------------------------------".cyan()); }
    
    wildcard_domains
}