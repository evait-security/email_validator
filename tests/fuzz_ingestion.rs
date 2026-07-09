mod common;

use assert_cmd::Command;
use common::{
    assert_sorted_and_unique, build_markdown_string, build_xml_string, create_input_file,
    create_input_file_raw,
};
use proptest::prelude::*;
use regex::Regex;

fn stdout_lines(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
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

// ── Markdown token strategy ────────────────────────────────────────

fn markdown_token_strategy() -> impl Strategy<Value = String> {
    let valid = (
        proptest::string::string_regex("[a-z0-9._%+-]{1,15}").unwrap(),
        proptest::string::string_regex("[a-z0-9.-]{2,10}").unwrap(),
        proptest::sample::select(vec!["de", "com", "org", "net", "io", "test", "example", "service", "docs", "test.de", "example.org"]),
    )
        .prop_map(|(local, domain, tld)| format!("{local}@{domain}.{tld}"));

    let deep_valid = (
        proptest::string::string_regex("[a-z0-9._%+-]{1,12}").unwrap(),
        proptest::string::string_regex("[a-z0-9-]{2,10}").unwrap(),
        proptest::string::string_regex("[a-z0-9-]{2,10}").unwrap(),
        proptest::sample::select(vec!["de", "com", "org", "net", "io", "fr", "uk", "us", "ca", "au", "jp", "cn", "br", "in"]),
    )
        .prop_map(|(local, s1, s2, tld)| format!("{local}@{s1}.{s2}.{tld}"));

    let invalid = proptest::strategy::Union::new(vec![
        proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("[a-z]{1,10}@").unwrap(),
        ),
        proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("@[a-z]{1,10}\\.[a-z]{2,4}").unwrap(),
        ),
        proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("[a-z]{1,10}@domain\\.[a-z]").unwrap(),
        ),
        proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("[a-z]{1,10}@\\.[a-z]{2,4}").unwrap(),
        ),
    ]);

    let valid_email = proptest::strategy::Union::new_weighted(vec![
        (5, proptest::strategy::Strategy::boxed(valid)),
        (3, proptest::strategy::Strategy::boxed(deep_valid)),
    ]);

    proptest::strategy::Union::new_weighted(vec![
        (12, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("- Contact: `{e}`")),
        )),
        (8, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("[Link](mailto:{e})")),
        )),
        (6, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("**{e}**")),
        )),
        (4, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("```text\n{e}\n```")),
        )),
        (4, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("## Heading {e}")),
        )),
        (3, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("> Blockquote: {e}")),
        )),
        (3, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("| Table | {e} |")),
        )),
        (3, proptest::strategy::Strategy::boxed(
            valid_email.prop_map(|e| format!("{e}\n{e}")),
        )),
        (5, proptest::strategy::Strategy::boxed(
            invalid.prop_map(|e| format!("- Broken: `{e}`")),
        )),
        (6, proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("[A-Za-z0-9 _*#\\->|`~]{0,50}")
                .unwrap()
                .prop_map(|s| s),
        )),
    ])
}

// ── XML token strategy ─────────────────────────────────────────────

fn xml_token_strategy() -> impl Strategy<Value = String> {
    let valid = (
        proptest::string::string_regex("[a-z0-9._%+-]{1,15}").unwrap(),
        proptest::string::string_regex("[a-z0-9.-]{2,10}").unwrap(),
        proptest::sample::select(vec!["de", "com", "org", "net", "io", "test"]),
    )
        .prop_map(|(local, domain, tld)| format!("{local}@{domain}.{tld}"));

    let deep_valid = (
        proptest::string::string_regex("[a-z0-9._%+-]{1,12}").unwrap(),
        proptest::string::string_regex("[a-z0-9-]{2,10}").unwrap(),
        proptest::string::string_regex("[a-z0-9-]{2,10}").unwrap(),
        proptest::sample::select(vec!["de", "com", "org"]),
    )
        .prop_map(|(local, s1, s2, tld)| format!("{local}@{s1}.{s2}.{tld}"));

    let invalid = proptest::strategy::Union::new(vec![
        proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("[a-z]{1,10}@").unwrap(),
        ),
        proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("@[a-z]{1,10}\\.[a-z]{2,4}").unwrap(),
        ),
        proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("[a-z]{1,10}@domain\\.[a-z]").unwrap(),
        ),
    ]);

    let valid_email = proptest::strategy::Union::new_weighted(vec![
        (5, proptest::strategy::Strategy::boxed(valid)),
        (3, proptest::strategy::Strategy::boxed(deep_valid)),
    ]);

    proptest::strategy::Union::new_weighted(vec![
        (10, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("<entry email=\"{e}\" type=\"person\">User</entry>")),
        )),
        (6, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("<entry><name>Test</name><email>{e}</email></entry>")),
        )),
        (5, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("<entry type=\"alias\"><![CDATA[{e}]]></entry>")),
        )),
        (5, proptest::strategy::Strategy::boxed(
            valid_email
                .clone()
                .prop_map(|e| format!("<note>Reachable: {e}</note>")),
        )),
        (4, proptest::strategy::Strategy::boxed(
            valid_email
                .prop_map(|e| format!("<contact primary=\"{e}\" secondary=\"{e}\"/>")),
        )),
        (4, proptest::strategy::Strategy::boxed(
            invalid
                .clone()
                .prop_map(|e| format!("<broken>{e}</broken>")),
        )),
        (3, proptest::strategy::Strategy::boxed(
            invalid.prop_map(|e| format!("<entry email=\"{e}\"/>")),
        )),
        (5, proptest::strategy::Strategy::boxed(
            proptest::string::string_regex("[A-Za-z0-9 _<>=/\\-]{0,50}")
                .unwrap()
                .prop_map(|s| format!("<raw>{s}</raw>")),
        )),
    ])
}

// ── Core proptest for mixed tokens (already existed, kept for coverage) ──

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 12,
        .. ProptestConfig::default()
    })]

    #[test]
    fn proptest_mixed_email_stream_invariants(tokens in proptest::collection::vec(mixed_token_strategy(), 1..180)) {
        let input = create_input_file(&tokens);
        let output = Command::cargo_bin("email_validator")
            .unwrap()
            .arg("-i")
            .arg(input.path())
            .arg("-m")
            .arg("regex")
            .arg("-f")
            .arg("list")
            .output()
            .unwrap();

        prop_assert!(output.status.success());

        let parsed = stdout_lines(&output.stdout);
        assert_sorted_and_unique(&parsed);

        let email_regex = Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").unwrap();
        for email in parsed {
            prop_assert!(email_regex.is_match(&email));
        }
    }
}

// ── Markdown fuzz tests ────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 15,
        .. ProptestConfig::default()
    })]

    /// Fuzz: Markdown tokens via temp file (line-based vector), CLI ingestion.
    #[test]
    fn proptest_markdown_token_stream_invariants(
        tokens in proptest::collection::vec(markdown_token_strategy(), 1..200)
    ) {
        let input = create_input_file(&tokens);
        let output = Command::cargo_bin("email_validator")
            .unwrap()
            .arg("-i")
            .arg(input.path())
            .arg("-m")
            .arg("regex")
            .arg("-f")
            .arg("list")
            .output()
            .unwrap();

        prop_assert!(output.status.success());

        let parsed = stdout_lines(&output.stdout);
        assert_sorted_and_unique(&parsed);

        let email_regex =
            Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").unwrap();
        for email in &parsed {
            prop_assert!(
                email_regex.is_match(email),
                "markdown fuzz: non-conforming email '{email}'"
            );
        }
    }

    /// Fuzz: Markdown as a single continuous string (no line breaks inserted).
    /// Simulates a real .md file with markdown syntax inline.
    #[test]
    fn proptest_markdown_continuous_string_invariants(
        entries in 20..200usize
    ) {
        let md_content = build_markdown_string(entries);
        let input = create_input_file_raw(&md_content);
        let output = Command::cargo_bin("email_validator")
            .unwrap()
            .arg("-i")
            .arg(input.path())
            .arg("-m")
            .arg("regex")
            .arg("-f")
            .arg("list")
            .output()
            .unwrap();

        prop_assert!(output.status.success());

        let parsed = stdout_lines(&output.stdout);
        assert_sorted_and_unique(&parsed);

        let email_regex =
            Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").unwrap();
        for email in &parsed {
            prop_assert!(email_regex.is_match(email));
        }

        // Known good emails must appear
        let set: std::collections::HashSet<&str> = parsed.iter().map(String::as_str).collect();
        prop_assert!(set.contains("dup.markdown@example.com"));
        prop_assert!(set.contains("dup.markdown@example.com") || set.contains("Dup.Markdown@Example.com"));
    }
}

// ── XML fuzz tests ─────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 15,
        .. ProptestConfig::default()
    })]

    /// Fuzz: XML tokens via temp file (line-based vector), CLI ingestion.
    #[test]
    fn proptest_xml_token_stream_invariants(
        tokens in proptest::collection::vec(xml_token_strategy(), 1..200)
    ) {
        let input = create_input_file(&tokens);
        let output = Command::cargo_bin("email_validator")
            .unwrap()
            .arg("-i")
            .arg(input.path())
            .arg("-m")
            .arg("regex")
            .arg("-f")
            .arg("list")
            .output()
            .unwrap();

        prop_assert!(output.status.success());

        let parsed = stdout_lines(&output.stdout);
        assert_sorted_and_unique(&parsed);

        let email_regex =
            Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").unwrap();
        for email in &parsed {
            prop_assert!(
                email_regex.is_match(email),
                "xml fuzz: non-conforming email '{email}'"
            );
        }
    }

    /// Fuzz: XML as a single continuous string (simulates a real .xml file).
    #[test]
    fn proptest_xml_continuous_string_invariants(entries in 20..200usize) {
        let xml_content = build_xml_string(entries);
        let input = create_input_file_raw(&xml_content);
        let output = Command::cargo_bin("email_validator")
            .unwrap()
            .arg("-i")
            .arg(input.path())
            .arg("-m")
            .arg("regex")
            .arg("-f")
            .arg("list")
            .output()
            .unwrap();

        prop_assert!(output.status.success());

        let parsed = stdout_lines(&output.stdout);
        assert_sorted_and_unique(&parsed);

        let email_regex =
            Regex::new(r"(?i)^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$").unwrap();
        for email in &parsed {
            prop_assert!(email_regex.is_match(email));
        }

        // Known good emails must appear
        let set: std::collections::HashSet<&str> = parsed.iter().map(String::as_str).collect();
        prop_assert!(set.contains("dup.xml@example.com"));
    }
}
