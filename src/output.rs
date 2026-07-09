//! Phase 3: Output Handling.
//!
//! Writes validated emails to a file or STDOUT in the requested format:
//!
//! | Format | Output |
//! |--------|--------|
//! | [`Format::List`] | One email per line, plain text |
//! | [`Format::Gophish`] | CSV with header: `First Name,Last Name,Email,Position` |
//!
//! Handles broken pipes gracefully by stopping the write loop on error
//! (e.g. when piped into `head`).

use std::fs::File;
use std::io::{self, Write};
use colored::*;
use crate::{Cli, Format};

/// Write validated emails to the output destination.
///
/// # Pipe safety
///
/// If the output stream breaks (e.g. `email_validator ... | head -5`),
/// the write loop exits silently without error.
pub fn run(cli: &Cli, output_data: &[String], is_quiet: bool) {
    let dest: Box<dyn Write> = match &cli.output {
        Some(output_file) => Box::new(File::create(output_file).unwrap_or_else(|e| {
            eprintln!("{}", format!("[!] Error creating output file: {}", e).red());
            std::process::exit(1);
        })),
        None => Box::new(io::stdout()),
    };

    match cli.format {
        Format::Gophish => {
            let mut wtr = csv::Writer::from_writer(dest);
            let _ = wtr.write_record(&["First Name", "Last Name", "Email", "Position"]);
            for email in output_data {
                if wtr.write_record(&["", "", email, ""]).is_err() { break; }
            }
            let _ = wtr.flush();
        }
        Format::List => {
            let mut dest = dest; 
            for email in output_data {
                if writeln!(dest, "{}", email).is_err() { break; }
            }
        }
    }

    if let Some(output_file) = &cli.output {
        if !is_quiet {
            eprintln!("{}", format!("[*] {} valid emails successfully processed and saved to '{}'.", output_data.len(), output_file).white());
        }
    }
}