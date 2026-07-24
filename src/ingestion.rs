//! Phase 0: Data Ingestion.
//!
//! Reads input from a file or STDIN, extracts all RFC-compliant email addresses
//! using a regex, then deduplicates and sorts the results.
//!
//! ## Supported Input Formats
//!
//! The regex parser reliably extracts emails from TXT, Markdown, XML, HTML,
//! CSV, and arbitrary mixed content — including code blocks, attributes,
//! `mailto:` links, and CDATA sections.

use std::fs::File;
use std::io::{self, Read};
use std::collections::HashSet;
use colored::*;
use regex::Regex;
use crate::RunArgs;

/// Maximum total input size in bytes (10 MB).
const MAX_INPUT_SIZE: u64 = 10 * 1024 * 1024;

/// Chunk size per read iteration (64 KB).
const CHUNK_SIZE: usize = 64 * 1024;

/// Extract all email addresses from a raw string using regex,
/// then sort and deduplicate the results.
///
/// # Regex
///
/// ```text
/// (?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}
/// ```
///
/// Matches the local-part, `@`, domain, and a TLD of at least 2 characters.
/// Case-insensitive matching is enabled.
///
/// # Examples
///
/// ```
/// use email_validator::ingestion::extract_and_deduplicate;
///
/// let result = extract_and_deduplicate("alice@example.com bob@test.de alice@example.com");
/// assert_eq!(result, vec!["alice@example.com", "bob@test.de"]);
/// ```
pub fn extract_and_deduplicate(input: &str) -> Vec<String> {
    let email_regex = Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}").unwrap();
    let mut extracted: Vec<String> = email_regex
        .find_iter(input)
        .map(|mat| mat.as_str().to_string())
        .collect();
    
    extracted.sort();
    extracted.dedup();
    extracted
}

/// Read input (file or STDIN), extract, deduplicate, and sort emails.
///
/// Reads in 64 KB chunks to avoid unbounded memory allocation. A hard limit
/// of 10 MB total input is enforced. Emails are extracted incrementally and
/// deduplicated in a [`HashSet`] to keep memory usage proportional to the
/// number of unique addresses, not the raw input size.
///
/// # Panics
///
/// Does not panic — exits the process gracefully with status 1 on file errors
/// or status 0 when no valid emails are found.
pub fn run(args: &RunArgs) -> Vec<String> {
    let email_regex = Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}").unwrap();
    let mut unique_emails: HashSet<String> = HashSet::new();
    let mut total_bytes_read: u64 = 0;
    let mut overflow_buffer = String::new();

    // Helper: extract emails from a chunk and insert into the set.
    let mut process_chunk = |chunk: &str| {
        for mat in email_regex.find_iter(chunk) {
            unique_emails.insert(mat.as_str().to_lowercase());
        }
    };

    if let Some(input_file) = &args.input {
        let mut file = match File::open(input_file) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("{}", "[!] Error: File not found".red());
                std::process::exit(1);
            }
        };

        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            let n = match file.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => n,
                Err(_) => {
                    eprintln!("{}", "[!] Error while reading the input file".red());
                    std::process::exit(1);
                }
            };
            total_bytes_read += n as u64;
            if total_bytes_read > MAX_INPUT_SIZE {
                eprintln!("{}", format!("[!] Input exceeds maximum size of {} MB", MAX_INPUT_SIZE / (1024 * 1024)).red());
                std::process::exit(1);
            }
            let chunk_str = String::from_utf8_lossy(&buf[..n]);
            // Prepend any overflow from the previous chunk (partial email at boundary).
            let combined = if overflow_buffer.is_empty() {
                chunk_str.into_owned()
            } else {
                let combined = overflow_buffer.clone() + &chunk_str;
                overflow_buffer.clear();
                combined
            };
            process_chunk(&combined);
            // Keep the last chunk's tail that might be a partial email
            // (everything after the last whitespace or newline).
            if let Some(last_ws) = combined.rfind(|c: char| c.is_whitespace()) {
                overflow_buffer = combined[last_ws..].to_string();
            }
        }
        // Process any leftover overflow.
        if !overflow_buffer.is_empty() {
            process_chunk(&overflow_buffer);
        }
    } else {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut had_input = false;
        loop {
            let n = match handle.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => n,
                Err(_) => break,
            };
            had_input = true;
            total_bytes_read += n as u64;
            if total_bytes_read > MAX_INPUT_SIZE {
                eprintln!("{}", format!("[!] Input exceeds maximum size of {} MB", MAX_INPUT_SIZE / (1024 * 1024)).red());
                std::process::exit(1);
            }
            let chunk_str = String::from_utf8_lossy(&buf[..n]);
            let combined = if overflow_buffer.is_empty() {
                chunk_str.into_owned()
            } else {
                let combined = overflow_buffer.clone() + &chunk_str;
                overflow_buffer.clear();
                combined
            };
            process_chunk(&combined);
            if let Some(last_ws) = combined.rfind(|c: char| c.is_whitespace()) {
                overflow_buffer = combined[last_ws..].to_string();
            }
        }
        if !overflow_buffer.is_empty() {
            process_chunk(&overflow_buffer);
        }
        if !had_input {
            eprintln!("No input found. Use -i or pipe data via STDIN.");
            std::process::exit(0);
        }
    }

    if unique_emails.is_empty() {
        eprintln!("{}", "[!] No valid emails found in input.".yellow());
        std::process::exit(0);
    }

    let mut sorted: Vec<String> = unique_emails.into_iter().collect();
    sorted.sort();
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn assert_sorted_and_unique(values: &[String]) {
        let mut expected = values.to_vec();
        expected.sort();
        expected.dedup();
        assert_eq!(values, expected);
    }

    #[test]
    fn test_extract_valid_emails_from_dirty_text() {
        let dirty_text = "Text: max@greenhats.com. Trash <info@test.de>.";
        let result = extract_and_deduplicate(dirty_text);
        
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "info@test.de");
        assert_eq!(result[1], "max@greenhats.com");
    }

    #[test]
    fn test_deduplication() {
        let text_with_duplicates = "test@domain.com, test@domain.com";
        let result = extract_and_deduplicate(text_with_duplicates);
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "test@domain.com");
    }

    #[test]
    fn test_no_emails_found() {
        let clean_text = "Just text, no addresses here.";
        let result = extract_and_deduplicate(clean_text);
        assert!(result.is_empty());
    }

    #[test]
    fn test_complex_email_extraction() {
        let text = "Contact us at first.last+tag@sub.domain.co.uk or admin@123-company.org.";
        let result = extract_and_deduplicate(text);
        
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "admin@123-company.org");
        assert_eq!(result[1], "first.last+tag@sub.domain.co.uk");
    }

    #[test]
    fn test_ignore_invalid_emails() {
        let text = "Bad emails: user@domain @domain.com user@.com just-a-string user@domain.c";
        let result = extract_and_deduplicate(text);
        
        assert!(result.is_empty());
    }

    #[test]
    fn test_regex_fuzzing_simulation() {
        let long_string = "A".repeat(10_000);

        let malicious_payloads = vec![
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa!",
            // SQL Injection & XSS 
            "' OR 1=1; DROP TABLE users; -- <script>alert(1)</script> user@hacker.com",
            // Memory / Buffer Check via reference
            &long_string,
            // Zalgo Text / Weird Unicode
            "u̸s̷e̸r̵@̶d̵o̸m̷a̸i̶n̵.̶c̸o̷m̷ 💥 ¯\\_(ツ)_/¯",
            // Brocken / Malformed Emails
            "@@@@@@.com user@.com.com.com user@domain..com",
        ];

        for payload in malicious_payloads {
            let result = extract_and_deduplicate(payload);
            
            // Ensure that the function does not panic and handles the input gracefully
            if payload.contains("user@hacker.com") {
                assert_eq!(result.len(), 1);
                assert_eq!(result[0], "user@hacker.com");
            }
        }
    }

    #[test]
    fn test_extract_from_txt_with_noise_and_duplicates() {
            let txt_like_content = r#"
                Contact list:
                - alice@example.com
                - alice@example.com
                - Alice@Example.com
                - zu.vor.nachnamen@lol.das.test.de
                Broken: user@ and @domain.tld and a..b@x.de
            "#;

            let result = extract_and_deduplicate(txt_like_content);

            assert_eq!(
                result,
                vec![
                    "Alice@Example.com".to_string(),
                    "a..b@x.de".to_string(),
                    "alice@example.com".to_string(),
                    "zu.vor.nachnamen@lol.das.test.de".to_string()
                ]
            );
        }

        #[test]
        fn test_extract_from_markdown_content_blocks() {
            let markdown_content = r#"
    # Team Contacts

    Primary: **lead.engineer@dept.example.org**

    - Backup: zu.vor.nachnamen@lol.das.test.de
    - Broken: user@ and @x.de

    ```txt
    alerts.ops+night@monitoring.deep.layer.test.de
    ```

    Find us at [Email](mailto:support@docs.example.com).
    "#;

            let result = extract_and_deduplicate(markdown_content);

            assert_eq!(
                result,
                vec![
                    "alerts.ops+night@monitoring.deep.layer.test.de".to_string(),
                    "lead.engineer@dept.example.org".to_string(),
                    "support@docs.example.com".to_string(),
                    "zu.vor.nachnamen@lol.das.test.de".to_string()
                ]
            );
        }

        #[test]
        fn test_extract_from_xml_nodes_and_attributes() {
            let xml_content = r#"
    <contacts>
      <owner email="owner.primary@service.test.de">Owner</owner>
      <entry><![CDATA[xml.cdata.person@a.b.c.example.net]]></entry>
      <note>reachable: contact+sales@dept.test.org</note>
      <bad>user@ domain@ invalid@.de</bad>
    </contacts>
    "#;

            let result = extract_and_deduplicate(xml_content);

            assert_eq!(
                result,
                vec![
                    "contact+sales@dept.test.org".to_string(),
                    "owner.primary@service.test.de".to_string(),
                    "xml.cdata.person@a.b.c.example.net".to_string()
                ]
            );
        }

        #[test]
        fn test_invalid_variants_and_case_duplicate_behavior() {
            let mixed = r#"
    valid@test.de
    Valid@Test.de
    user@
    @domain.tld
    name@domain.c
    broken@@domain.de
            "#;

            let result = extract_and_deduplicate(mixed);

            assert_eq!(result, vec!["Valid@Test.de".to_string(), "valid@test.de".to_string()]);
        }

        fn mixed_token_strategy() -> impl Strategy<Value = String> {
            let valid_basic = (
                proptest::string::string_regex("[a-z]{1,12}").unwrap(),
                proptest::string::string_regex("[a-z]{2,10}").unwrap(),
                proptest::sample::select(vec!["de", "com", "org", "net", "io"]),
            )
                .prop_map(|(local, domain, tld)| format!("{local}@{domain}.{tld}"));

            let valid_deep = (
                proptest::string::string_regex("[a-z]{1,10}(\\.[a-z]{1,10}){0,2}").unwrap(),
                proptest::string::string_regex("[a-z]{2,10}").unwrap(),
                proptest::string::string_regex("[a-z]{2,10}").unwrap(),
                proptest::sample::select(vec!["de", "com", "org"]),
            )
                .prop_map(|(local, s1, s2, tld)| format!("{local}@{s1}.{s2}.{tld}"));

            let invalid = prop_oneof![
                proptest::string::string_regex("[a-z]{1,10}@").unwrap(),
                proptest::string::string_regex("@[a-z]{1,10}\\.[a-z]{2,4}").unwrap(),
                proptest::string::string_regex("[a-z]{1,10}@@[a-z]{1,10}\\.[a-z]{2,4}").unwrap(),
                proptest::string::string_regex("[a-z]{1,10}@\\.[a-z]{2,4}").unwrap(),
                proptest::string::string_regex("[a-z]{1,10}@domain\\.[a-z]").unwrap(),
            ];

            let noisy_text = proptest::string::string_regex("[A-Za-z0-9 _<>=:/#\\-\\.,]{0,40}").unwrap();

            prop_oneof![5 => valid_basic, 3 => valid_deep, 3 => invalid, 4 => noisy_text]
        }

    proptest! {
        #[test]
        fn proptest_mixed_email_stream_invariants(tokens in proptest::collection::vec(mixed_token_strategy(), 0..300)) {
            let input = tokens.join("\n");
            let result = extract_and_deduplicate(&input);
            let email_regex = Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").unwrap();

            assert_sorted_and_unique(&result);
            for email in &result {
                prop_assert!(email_regex.is_match(email));
            }
        }
    }
}