//! Phase 3: Output Handling.
//!
//! Writes validation results to a file or STDOUT in the requested format:
//!
//! | Flag | Output |
//! |------|--------|
//! | `-f list` (default) | One valid email per line, plain text |
//! | `-f gophish` | CSV with header: `First Name,Last Name,Email,Position` |
//! | `-j` / `--json` | JSON array of all results (valid + invalid), e.g. `[{"email":"a@b.de","valid":true}]` |
//!
//! List and Gophish formats filter to valid-only results. JSON includes every
//! email from the input, with `valid` and (optionally) `catch_all` fields.
//!
//! Handles broken pipes gracefully by stopping the write loop on error
//! (e.g. when piped into `head`).

use std::fs::File;
use std::io::{self, Write};
use colored::*;
use crate::{Cli, Format};
use crate::validation::ValidationResult;

/// Write validation results to the output destination.
///
/// # Pipe safety
///
/// If the output stream breaks (e.g. `email_validator ... | head -5`),
/// the write loop exits silently without error.
pub fn run(cli: &Cli, output_data: &[ValidationResult], is_quiet: bool) {
    let mut dest: Box<dyn Write> = match &cli.output {
        Some(output_file) => Box::new(File::create(output_file).unwrap_or_else(|e| {
            eprintln!("{}", format!("[!] Error creating output file: {}", e).red());
            std::process::exit(1);
        })),
        None => Box::new(io::stdout()),
    };

    if cli.json {
        // JSON output: serialize ALL results (valid + invalid)
        if serde_json::to_writer_pretty(&mut dest, output_data).is_err() {
            // Pipe broken — exit silently
        }
        let _ = writeln!(dest);
    } else {
        match cli.format {
            Format::Gophish => {
                let mut wtr = csv::Writer::from_writer(&mut dest);
                let _ = wtr.write_record(["First Name", "Last Name", "Email", "Position"]);
                for result in output_data {
                    if !result.valid { continue; }
                    if wtr.write_record(["", "", &result.email, ""]).is_err() { break; }
                }
                let _ = wtr.flush();
            }
            Format::List => {
                for result in output_data {
                    if !result.valid { continue; }
                    if writeln!(dest, "{}", result.email).is_err() { break; }
                }
            }
        }
    }

    if let Some(output_file) = &cli.output
        && !is_quiet
    {
        let valid_count = output_data.iter().filter(|r| r.valid).count();
        eprintln!("{}", format!("[*] {} valid emails of {} successfully processed and saved to '{}'.", valid_count, output_data.len(), output_file).white());
    }
}