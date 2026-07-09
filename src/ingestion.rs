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
use colored::*;
use regex::Regex;
use crate::Cli;

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
/// # Panics
///
/// Does not panic — exits the process gracefully with status 1 on file errors
/// or status 0 when no valid emails are found.
pub fn run(cli: &Cli) -> Vec<String> {
    let mut raw_input = String::new();
    
    if let Some(input_file) = &cli.input {
        if let Ok(mut file) = File::open(input_file) {
            file.read_to_string(&mut raw_input).unwrap_or_else(|_| {
                eprintln!("{}", "[!] Error while reading the input file".red());
                std::process::exit(1);
            });
        } else {
            eprintln!("{}", "[!] Error: File not found".red());
            std::process::exit(1);
        }
    } else {
        if io::stdin().read_to_string(&mut raw_input).is_ok() {
            if raw_input.trim().is_empty() {
                eprintln!("No input found. Use -i or pipe data via STDIN.");
                std::process::exit(0);
            }
        }
    }

    let unique_emails = extract_and_deduplicate(&raw_input);

    if unique_emails.is_empty() {
        eprintln!("{}", "[!] No valid emails found in input.".yellow());
        std::process::exit(0);
    }

    unique_emails
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