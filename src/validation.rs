//! Phase 2: Email Validation.
//!
//! Validates each deduplicated email address using the selected method:
//!
//! | Method | Behavior |
//! |--------|----------|
//! | [`Method::Regex`] | Syntax check via RFC-compliant regex |
//! | [`Method::Mx`] | Performs DNS MX record lookup via Hickory resolver |
//! | [`Method::Smtp`] | Full SMTP handshake via `check-if-email-exists` |
//!
//! If a domain was flagged as catch-all in [`precheck`](crate::precheck), valid
//! emails from that domain are marked with a warning but still included in output.
//!
//! # Output Structure
//!
//! Each email, regardless of validity, is returned as a [`ValidationResult`].
//! This enables the JSON output format (`-j`) to include both valid and invalid
//! emails, while the legacy list/gophish formats filter to valid-only in
//! [`output`](crate::output).

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::LazyLock;
use colored::*;
use check_if_email_exists::{check_email, CheckEmailInputBuilder, Reachable};
use hickory_resolver::TokioAsyncResolver;
use regex::Regex;
use serde::Serialize;
use crate::Method;

/// Compiled regex for email syntax validation (same as ingestion).
static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").unwrap()
});

/// Result of validating a single email address.
///
/// Returned by [`run`] for every email in the input list. The [`valid`]
/// field indicates whether the address passed the chosen validation method.
/// For SMTP mode, [`catch_all`] is `true` when the domain was flagged as a
/// catch-all / wildcard domain in [`precheck`](crate::precheck).
///
/// # JSON Serialization
///
/// When serialized via `serde_json`, the `catch_all` field is omitted if
/// `false` to keep output compact:
///
/// ```json
/// {"email":"a@b.de","valid":true,"catch_all":true}
/// {"email":"bogus","valid":false}
/// ```
#[derive(Serialize, Clone, Debug)]
pub struct ValidationResult {
    /// The email address that was validated.
    pub email: String,
    /// `true` if the address passed validation.
    pub valid: bool,
    /// `true` if the domain is a catch-all (only set in SMTP mode).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub catch_all: bool,
}

/// Returns `true` if the IP address belongs to a private, loopback, or
/// link-local range as defined by IETF RFC 1918, RFC 6598, RFC 5735, and
/// RFC 3927.
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            octets[0] == 10                                                     // 10.0.0.0/8
                || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)    // 172.16.0.0/12
                || (octets[0] == 192 && octets[1] == 168)                       // 192.168.0.0/16
                || octets[0] == 127                                            // 127.0.0.0/8 (loopback)
                || (octets[0] == 169 && octets[1] == 254)                      // 169.254.0.0/16 (link-local)
                || (octets[0] == 100 && octets[1] >= 64 && octets[1] <= 127)   // 100.64.0.0/10 (CGN)
                || octets == [0, 0, 0, 0]                                      // 0.0.0.0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()                 // ::1
                || v6.is_unique_local()      // fc00::/7
                || v6.is_unicast_link_local() // fe80::/10
        }
    }
}

/// Resolve the A/AAAA records for a hostname and return `true` if any
/// resolved IP falls within a private network range.
async fn resolves_to_private(hostname: &str, resolver: &TokioAsyncResolver) -> bool {
    if let Ok(lookup) = resolver.lookup_ip(hostname).await {
        lookup.iter().any(is_private_ip)
    } else {
        // If we can't resolve it, treat as safe (the SMTP handshake will fail anyway)
        false
    }
}

/// Validate each email and return a result for every input address.
///
/// Unlike previous versions, this now returns ALL emails (valid and invalid)
/// wrapped in a [`ValidationResult`], so that JSON output can include failed
/// addresses. Legacy list/gophish formats filter in [`output`](crate::output).
///
/// # Parameters
///
/// * `method` — Validation method (`Regex`, `Mx`, or `Smtp`)
/// * `disable_wildcard` — Skip wildcard-flagging if true
/// * `unique_emails` — Deduplicated list from Phase 0
/// * `wildcard_domains` — Set of catch-all domains from Phase 1
/// * `is_quiet` — Suppress per-email STDERR output if true
///
/// # Returns
///
/// A `Vec<ValidationResult>` for every input email, in original order.
pub async fn run(method: Method, disable_wildcard: bool, unique_emails: &[String], wildcard_domains: &HashSet<String>, is_quiet: bool, _concurrency: Option<usize>) -> Vec<ValidationResult> {
    if !is_quiet { eprintln!("{}", format!("==> Phase 2: Validating Email List using method: {:?}...", method).cyan()); }
    
    let mut output_data: Vec<ValidationResult> = Vec::new();
    let resolver_opt = if method == Method::Mx || method == Method::Smtp {
        Some(TokioAsyncResolver::tokio_from_system_conf().unwrap_or_else(|_| {
            TokioAsyncResolver::tokio(
                hickory_resolver::config::ResolverConfig::default(),
                hickory_resolver::config::ResolverOpts::default(),
            )
        }))
    } else {
        None
    };

    for email in unique_emails {
        let domain = email.split('@').nth(1).unwrap_or("");
        let mut is_valid = false;
        let mut is_catch_all_warning = false;

        match method {
            Method::Regex => {
                is_valid = EMAIL_RE.is_match(email);
            },
            Method::Mx => {
                if let Some(resolver) = &resolver_opt
                    && let Ok(mx_records) = resolver.mx_lookup(domain).await
                {
                    is_valid = mx_records.iter().next().is_some();
                }
            },
            Method::Smtp => {
                // SSRF guard: check if domain's MX hosts resolve to private IPs.
                // We use the same resolver that was initialised for MX lookups.
                if let Some(resolver) = &resolver_opt
                    && resolves_to_private(domain, resolver).await
                {
                    if !is_quiet {
                        eprintln!("{}", format!("[-] Skipped {} (MX resolves to private IP)", email).yellow());
                    }
                    output_data.push(ValidationResult { email: email.clone(), valid: false, catch_all: false });
                    continue;
                }

                let check_input = match CheckEmailInputBuilder::default().to_email(email.clone()).build() {
                    Ok(input) => input,
                    Err(_) => {
                        output_data.push(ValidationResult { email: email.clone(), valid: false, catch_all: false });
                        continue;
                    }
                };
                let result = check_email(&check_input).await;

                match result.is_reachable {
                    Reachable::Safe | Reachable::Risky => {
                        is_valid = true;
                        if !disable_wildcard && wildcard_domains.contains(domain) {
                            is_catch_all_warning = true;
                        }
                    },
                    _ => { is_valid = false; }
                }
            }
        }

        output_data.push(ValidationResult {
            email: email.clone(),
            valid: is_valid,
            catch_all: is_catch_all_warning,
        });

        if !is_quiet {
            if is_valid && is_catch_all_warning {
                eprintln!("{}", format!("[+] {} (Warning: Domain is Catch-All)", email).yellow());
            } else if is_valid {
                eprintln!("{}", format!("[+] {}", email).green());
            } else {
                eprintln!("{}", format!("[-] {}", email).red());
            }
        }
    }
    
    output_data
}