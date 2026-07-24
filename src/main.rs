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

use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;

pub mod ingestion;
pub mod precheck;
pub mod validation;
pub mod output;
pub mod server;

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
/// email_validator run -i mails.txt -m regex
///
/// # SMTP validation with GoPhish CSV output
/// email_validator run -i mails.txt -o out.csv -f gophish
///
/// # JSON output for scripting / automation pipelines
/// email_validator run -i mails.txt -j | jq
///
/// # Read from STDIN, write to file
/// cat mails.txt | email_validator run -o clean.txt
/// ```
#[derive(Parser, Debug)]
#[command(name = "verify")]
#[command(about = "E-Mail list validator written in Rust", long_about = None)]
#[command(after_help = "Examples:\n  # CLI pipeline: default config (smtp validation, list output)\n  email_validator run -i input_emails.txt -o verified_emails.txt\n\n  # smtp validation and csv gophish output\n  email_validator run -i input_emails.txt -o verified_emails.csv -f gophish\n\n  # regex validation only and csv gophish output\n  email_validator run -i input_emails.txt -o verified_emails.csv -f gophish -m regex\n\n  # JSON output for scripting / automation\n  email_validator run -i input_emails.txt -j -o result.json\n\n  # using the pipe, no output file, only stdout\n  cat /tmp/mails.txt | email_validator run -f gophish\n\n  # Start API server (default: 0.0.0.0:8080)\n  email_validator api\n\n  # API server on custom port\n  email_validator api 127.0.0.1:3000\n\n  # API server via env var\n  BIND_ADDR=0.0.0.0:9000 email_validator api")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the CLI pipeline (file/STDIN in → file/STDOUT out)
    Run(RunArgs),
    /// Start HTTP API server for email validation
    Api(ApiArgs),
}

#[derive(Parser, Debug)]
pub struct RunArgs {
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

#[derive(Parser, Debug)]
pub struct ApiArgs {
    /// Bind address. Defaults to 0.0.0.0:8080, overridable via $BIND_ADDR.
    #[arg(default_value = "0.0.0.0:8080", env = "BIND_ADDR")]
    pub bind_addr: String,
    /// Verbose STDERR logging
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
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, serde::Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
/// 1. Parse CLI subcommand (`run` or `api`)
/// 2. `run`: ingest → precheck → validate → output
/// 3. `api`: start HTTP server
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run(args) => {
            let is_quiet = (args.format == Format::Gophish || args.json) && !args.verbose;

            // Orchestrator: Semantic and clean function calls
            let unique_emails = ingestion::run(&args);
            let wildcard_domains = precheck::run(args.method, args.disable_wildcard, args.verbose, &unique_emails, is_quiet, None).await;
            let validation_results = validation::run(args.method, args.disable_wildcard, &unique_emails, &wildcard_domains, is_quiet, None).await;
            
            output::run(&args, &validation_results, is_quiet);
        }
        Command::Api(args) => {
            server::run(&args.bind_addr, args.verbose).await
        }
    }
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
        let args = vec!["email_validator", "run", "-i", "list.txt"];
        let cli = Cli::parse_from(args);
        let Command::Run(args) = cli.command else { panic!("expected Run") };
        
        assert_eq!(args.input.unwrap(), "list.txt");
        assert_eq!(args.output, None);
        assert_eq!(args.format, Format::List);
        assert!(!args.json);
        assert_eq!(args.method, Method::Smtp);
        assert_eq!(args.disable_wildcard, false);
        assert_eq!(args.verbose, false);
    }

    #[test]
    fn test_cli_custom_flags_and_options() {
        let args = vec![
            "email_validator", "run",
            "-i", "in.txt", 
            "-o", "out.csv", 
            "-f", "gophish", 
            "-m", "regex", 
            "-d", 
            "-v"
        ];
        let cli = Cli::parse_from(args);
        let Command::Run(args) = cli.command else { panic!("expected Run") };
        
        assert_eq!(args.input.unwrap(), "in.txt");
        assert_eq!(args.output.unwrap(), "out.csv");
        assert_eq!(args.format, Format::Gophish);
        assert!(!args.json);
        assert_eq!(args.method, Method::Regex);
        assert!(args.disable_wildcard);
        assert!(args.verbose);
    }

    #[test]
    fn test_cli_json_flag() {
        let args = vec![
            "email_validator", "run",
            "-i", "in.txt", 
            "-j",
            "-m", "regex",
        ];
        let cli = Cli::parse_from(args);
        let Command::Run(args) = cli.command else { panic!("expected Run") };
        
        assert_eq!(args.input.unwrap(), "in.txt");
        assert!(args.json);
        assert_eq!(args.format, Format::List);
        assert_eq!(args.method, Method::Regex);
    }

    #[test]
    fn test_cli_json_conflicts_with_format() {
        let args = vec![
            "email_validator", "run",
            "-i", "in.txt", 
            "-j",
            "-f", "gophish",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_api_args_default() {
        let args = vec!["email_validator", "api", "127.0.0.1:8080"];
        let cli = Cli::parse_from(args);
        let Command::Api(args) = cli.command else { panic!("expected Api") };
        assert_eq!(args.bind_addr, "127.0.0.1:8080");
        assert!(!args.verbose);
    }

    #[test]
    fn test_cli_api_args_verbose() {
        let args = vec!["email_validator", "api", "0.0.0.0:3000", "-v"];
        let cli = Cli::parse_from(args);
        let Command::Api(args) = cli.command else { panic!("expected Api") };
        assert_eq!(args.bind_addr, "0.0.0.0:3000");
        assert!(args.verbose);
    }
}