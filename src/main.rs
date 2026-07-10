//! # Email Validator
//!
//! A fast, statically-linkable email list validator written in Rust.
//!
//! ## Architecture
//!
//! The program runs in four phases:
//!
//! | Phase | Module | Description |
//! |-------|--------|-------------|
//! | 0 | [`ingestion`] | Read input (file or STDIN), extract, deduplicate, sort emails via regex |
//! | 1 | [`precheck`] | Wildcard / catch-all domain detection (SMTP mode only) |
//! | 2 | [`validation`] | Validate each email via regex, MX lookup, or full SMTP handshake |
//! | 3 | [`output`] | Write results as plain list or GoPhish CSV to file or STDOUT |
//!
//! ## Quick Example
//!
//! ```bash
//! email_validator -i input.txt -o verified.txt -m regex -f list
//! ```
//!
//! ## Generating Docs
//!
//! ```bash
//! cargo doc --no-deps --open
//! ```

use clap::{Parser, ValueEnum};

pub mod ingestion;
pub mod precheck;
pub mod validation;
pub mod output;

/// Command-line interface definition.
///
/// All options are parsed via [`clap`] and flow into the four processing phases.
///
/// # Fields
///
/// | Flag | Field | Description |
/// |------|-------|-------------|
/// | `-i` | `input` | Input file path (optional, reads STDIN if omitted) |
/// | `-o` | `output` | Output file path (optional, writes to STDOUT if omitted) |
/// | `-f` | `format` | Output format: [`Format::List`] or [`Format::Gophish`] |
/// | `-j` | `json` | Output as JSON array (conflicts with `-f`) |
/// | `-m` | `method` | Validation method: [`Method::Regex`], [`Method::Mx`], or [`Method::Smtp`] |
/// | `-d` | `disable_wildcard` | Skip wildcard domain detection |
/// | `-v` | `verbose` | Enable verbose progress output |
///
/// # Examples
///
/// ```bash
/// # Simple regex validation, list output
/// email_validator -i mails.txt -m regex
///
/// # SMTP validation with GoPhish CSV output
/// email_validator -i mails.txt -o out.csv -f gophish
///
/// # JSON output for n8n / automation pipelines
/// email_validator -i mails.txt -j | jq
///
/// # Read from STDIN, write to file
/// cat mails.txt | email_validator -o clean.txt
/// ```
#[derive(Parser, Debug)]
#[command(name = "verify")]
#[command(about = "E-Mail list validator written in Rust", long_about = None)]
#[command(after_help = "Examples:\n  # default config (smtp validation and list output)\n  email_validator -i input_emails.txt -o verified_emails.txt\n\n  # smtp validation and csv gophish output\n  email_validator -i input_emails.txt -o verified_emails.csv -f gophish\n\n  # regex validation only and csv gophish output\n  email_validator -i input_emails.txt -o verified_emails.csv -f gophish -m regex\n\n  # JSON output for n8n / automation\n  email_validator -i input_emails.txt -j -o result.json\n\n  # using the pipe, no output file, only stdout\n  cat /tmp/mails.txt | email_validator -f gophish")]
pub struct Cli {
    /// Input file path. Reads from STDIN if not provided.
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file path. Writes to STDOUT if not provided.
    #[arg(short, long)]
    pub output: Option<String>,

    /// Output format: `list` (one per line) or `gophish` (CSV).
    #[arg(short, long, value_enum, default_value_t = Format::List)]
    pub format: Format,

    /// Output as JSON array. Conflicts with `--format`.
    #[arg(short = 'j', long, conflicts_with = "format")]
    pub json: bool,

    /// Validation method: `regex`, `mx`, or `smtp`.
    #[arg(short, long, value_enum, default_value_t = Method::Smtp)]
    pub method: Method,

    /// Disable wildcard / catch-all domain detection.
    #[arg(short, long)]
    pub disable_wildcard: bool,

    /// Print detailed progress to STDERR.
    #[arg(short, long)]
    pub verbose: bool,
}

/// Output format for validated emails.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum Format {
    /// GoPhish-compatible CSV: `First Name,Last Name,Email,Position`
    Gophish,
    /// Plain text list: one email address per line.
    List,
}

/// Validation method used in Phase 2.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum Method {
    /// Syntax check only via RFC-compliant regex. No network required.
    Regex,
    /// Regex + DNS MX record lookup via Hickory resolver.
    Mx,
    /// Regex + MX + full SMTP handshake via `check-if-email-exists`.
    Smtp,
}

/// Program entry point.
///
/// 1. Parse CLI
/// 2. [`ingestion::run`] — extract deduplicated emails
/// 3. [`precheck::run`] — detect wildcard domains
/// 4. [`validation::run`] — validate emails (returns `Vec<ValidationResult>`)
/// 5. [`output::run`] — write results
#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let is_quiet = (cli.format == Format::Gophish || cli.json) && !cli.verbose;

    // Orchestrator: Semantic and clean function calls
    let unique_emails = ingestion::run(&cli);
    let wildcard_domains = precheck::run(&cli, &unique_emails, is_quiet).await;
    let validation_results = validation::run(&cli, &unique_emails, &wildcard_domains, is_quiet).await;
    
    output::run(&cli, &validation_results, is_quiet);
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    fn test_cli_structure_validity() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_cli_default_values() {
        let args = vec!["email_validator", "-i", "list.txt"];
        let cli = Cli::parse_from(args);
        
        assert_eq!(cli.input.unwrap(), "list.txt");
        assert_eq!(cli.output, None);
        assert_eq!(cli.format, Format::List);
        assert!(!cli.json);
        assert_eq!(cli.method, Method::Smtp);
        assert_eq!(cli.disable_wildcard, false);
        assert_eq!(cli.verbose, false);
    }

    #[test]
    fn test_cli_custom_flags_and_options() {
        let args = vec![
            "email_validator", 
            "-i", "in.txt", 
            "-o", "out.csv", 
            "-f", "gophish", 
            "-m", "regex", 
            "-d", 
            "-v"
        ];
        let cli = Cli::parse_from(args);
        
        assert_eq!(cli.input.unwrap(), "in.txt");
        assert_eq!(cli.output.unwrap(), "out.csv");
        assert_eq!(cli.format, Format::Gophish);
        assert!(!cli.json);
        assert_eq!(cli.method, Method::Regex);
        assert!(cli.disable_wildcard);
        assert!(cli.verbose);
    }

    #[test]
    fn test_cli_json_flag() {
        let args = vec![
            "email_validator", 
            "-i", "in.txt", 
            "-j",
            "-m", "regex",
        ];
        let cli = Cli::parse_from(args);
        
        assert_eq!(cli.input.unwrap(), "in.txt");
        assert!(cli.json);
        assert_eq!(cli.format, Format::List);
        assert_eq!(cli.method, Method::Regex);
    }

    #[test]
    fn test_cli_json_conflicts_with_format() {
        let args = vec![
            "email_validator", 
            "-i", "in.txt", 
            "-j",
            "-f", "gophish",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
    }
}