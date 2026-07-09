//! Phase 2: Email Validation.
//!
//! Validates each deduplicated email address using the selected method:
//!
//! | Method | Behavior |
//! |--------|----------|
//! | [`Method::Regex`] | Always passes (syntax already verified in ingestion) |
//! | [`Method::Mx`] | Performs DNS MX record lookup via Hickory resolver |
//! | [`Method::Smtp`] | Full SMTP handshake via `check-if-email-exists` |
//!
//! If a domain was flagged as catch-all in [`precheck`](crate::precheck), valid
//! emails from that domain are marked with a warning but still included in output.

use std::collections::HashSet;
use colored::*;
use check_if_email_exists::{check_email, CheckEmailInputBuilder, Reachable};
use hickory_resolver::TokioAsyncResolver;
use crate::{Cli, Method};

/// Validate each email and return only those that pass.
///
/// # Parameters
///
/// * `cli` — CLI arguments (determines method, wildcard flag)
/// * `unique_emails` — Deduplicated list from Phase 0
/// * `wildcard_domains` — Set of catch-all domains from Phase 1
/// * `is_quiet` — Suppress per-email STDERR output if true
///
/// # Returns
///
/// A `Vec<String>` of emails that passed validation, in original order.
pub async fn run(cli: &Cli, unique_emails: &[String], wildcard_domains: &HashSet<String>, is_quiet: bool) -> Vec<String> {
    if !is_quiet { eprintln!("{}", format!("==> Phase 2: Validating Email List using method: {:?}...", cli.method).cyan()); }
    
    let mut output_data: Vec<String> = Vec::new();
    let resolver_opt = if cli.method == Method::Mx {
        Some(TokioAsyncResolver::tokio_from_system_conf().expect("Failed to initialize DNS resolver"))
    } else {
        None
    };

    for email in unique_emails {
        let domain = email.split('@').nth(1).unwrap_or("");
        let mut is_valid = false;
        let mut is_catch_all_warning = false;

        match cli.method {
            Method::Regex => {
                is_valid = true;
            },
            Method::Mx => {
                if let Some(resolver) = &resolver_opt {
                    if let Ok(mx_records) = resolver.mx_lookup(domain).await {
                        is_valid = mx_records.iter().next().is_some();
                    }
                }
            },
            Method::Smtp => {
                let check_input = CheckEmailInputBuilder::default().to_email(email.clone()).build().unwrap();
                let result = check_email(&check_input).await;

                match result.is_reachable {
                    Reachable::Safe | Reachable::Risky => {
                        is_valid = true;
                        if !cli.disable_wildcard && wildcard_domains.contains(domain) {
                            is_catch_all_warning = true;
                        }
                    },
                    _ => { is_valid = false; }
                }
            }
        }

        if is_valid {
            output_data.push(email.clone());
            if !is_quiet {
                if is_catch_all_warning {
                    eprintln!("{}", format!("[+] {} (Warning: Domain is Catch-All)", email).yellow());
                } else {
                    eprintln!("{}", format!("[+] {}", email).green());
                }
            }
        } else {
            if !is_quiet { eprintln!("{}", format!("[-] {}", email).red()); }
        }
    }
    
    output_data
}