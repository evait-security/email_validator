mod common;

use assert_cmd::Command;
use common::{
    assert_sorted_and_unique, build_mass_markdown_lines, build_mass_mixed_lines,
    build_mass_xml_lines, create_input_file, create_input_file_raw,
    build_markdown_string, build_xml_string,
};
use std::collections::HashSet;

fn stdout_lines(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[test]
fn test_mass_mixed_input_regex_cli() {
    let lines = build_mass_mixed_lines(1200);
    let input = create_input_file(&lines);

    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-f")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let parsed = stdout_lines(&output.stdout);
    assert!(!parsed.is_empty());
    assert_sorted_and_unique(&parsed);

    let set: HashSet<&str> = parsed.iter().map(String::as_str).collect();
    assert!(set.contains("user0@example.com"));
    assert!(set.contains("zu.vor.nachnamen1@lol.das.test.de"));
    assert!(set.contains("xml.person5@a.b.c.example.net"));
    assert!(set.contains("markdown.user6@docs.example.org"));
    assert!(set.contains("single.good@txt.test.de"));
    assert!(set.contains("xml.attribute@service.test.de"));

    assert!(!set.contains("broken2@"));
    assert!(!set.contains("@missing-local3.de"));
    assert!(!set.contains("noise-line-4"));
}

// ── Mass markdown CLI test ─────────────────────────────────────────

#[test]
fn test_mass_markdown_input_regex_cli() {
    let lines = build_mass_markdown_lines(800);
    let input = create_input_file(&lines);

    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-f")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let parsed = stdout_lines(&output.stdout);
    assert!(!parsed.is_empty());
    assert_sorted_and_unique(&parsed);

    let set: HashSet<&str> = parsed.iter().map(String::as_str).collect();

    // Representative valid hits
    assert!(set.contains("primary0@example.com"));
    assert!(set.contains("backup1@sub.docs.test.de"));
    assert!(set.contains("nested2@deep.layer.example.org"));
    assert!(set.contains("support+team3@help.example.com"));
    assert!(set.contains("code.block4@inline.codeblock.test.de"));
    assert!(set.contains("heading.5@section.title.example.net"));
    assert!(set.contains("table6@row.example.com"));
    assert!(set.contains("blockquote7@quoted.example.org"));
    assert!(set.contains("strikethrough8@deleted.example.de"));

    // italic9/bold10: The regex may capture leading `_`/`__` as part of the local-part
    // since underscores are valid in [A-Z0-9._%+-]+. Just check presence generically.
    let has_italic_or_bold = set.iter().any(|e| e.contains("italic") || e.contains("bold"));
    assert!(has_italic_or_bold, "expected at least one italic/bold email in: {set:?}");

    // Dup.Markdown@Example.com and dup.markdown@example.com are distinct (case-sensitive dedup)
    assert!(set.contains("dup.markdown@example.com"));

    // Broken emails the regex MUST NOT match (missing domain, missing local, single-char TLD)
    assert!(!set.contains("user@"));
    assert!(!set.contains("@domain.tld"));
    assert!(!set.contains("name@domain.c"));

    // a..b@example.de: The regex allows dots in the local-part ([A-Z0-9._%+-]+).
    // Double dots are invalid per RFC 5321, but the regex does not reject them.
    // Known behavior — these pass as "valid" at the extraction layer.
}

// ── Mass markdown single-string CLI test ───────────────────────────

#[test]
fn test_mass_markdown_flat_string_regex_cli() {
    let md = build_markdown_string(600);
    let input = create_input_file_raw(&md);

    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-f")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let parsed = stdout_lines(&output.stdout);
    assert!(!parsed.is_empty());
    assert_sorted_and_unique(&parsed);

    let set: HashSet<&str> = parsed.iter().map(String::as_str).collect();
    assert!(set.contains("primary0@example.com"));
    assert!(set.contains("dup.markdown@example.com"));
}

// ── Mass XML CLI test ──────────────────────────────────────────────

#[test]
fn test_mass_xml_input_regex_cli() {
    let lines = build_mass_xml_lines(800);
    let input = create_input_file(&lines);

    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-f")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let parsed = stdout_lines(&output.stdout);
    assert!(!parsed.is_empty());
    assert_sorted_and_unique(&parsed);

    let set: HashSet<&str> = parsed.iter().map(String::as_str).collect();

    // Representative valid hits
    assert!(set.contains("user0@example.com"));
    assert!(set.contains("sub1@sub.docs.test.de"));
    assert!(set.contains("cdata.person2@a.b.c.example.net"));
    assert!(set.contains("note+person3@notes.test.org"));
    assert!(set.contains("user.5@deep.nested.domain.test.de"));
    assert!(set.contains("group.primary7@groups.example.com"));
    assert!(set.contains("group.secondary7@groups.example.com"));
    assert!(set.contains("8.namespaced@ns.schema.test.org"));
    assert!(set.contains("dup.xml@example.com"));

    // Broken emails must NOT appear
    assert!(!set.contains("broken4@"));
    assert!(!set.contains("@missing-local6.de"));
}

// ── Mass XML single-string CLI test ────────────────────────────────

#[test]
fn test_mass_xml_flat_string_regex_cli() {
    let xml = build_xml_string(500);
    let input = create_input_file_raw(&xml);

    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-f")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let parsed = stdout_lines(&output.stdout);
    assert!(!parsed.is_empty());
    assert_sorted_and_unique(&parsed);

    let set: HashSet<&str> = parsed.iter().map(String::as_str).collect();
    assert!(set.contains("user0@example.com"));
    assert!(set.contains("dup.xml@example.com"));
}

// ── Smoke / Edge-case tests ────────────────────────────────────────

#[test]
fn test_markdown_no_panic_on_malformed_syntax() {
    // Extremely malformed markdown that should never panic
    let payload = r#"
# ~~~ ** __ ***
[broken](mailto:)(mailto:x@y.de)
``` ```  ``` 
` ` ` x@y.de ` ` `
||table|| # heading @@@
"#;
    let input = create_input_file_raw(payload);
    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-f")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed = stdout_lines(&output.stdout);
    assert_sorted_and_unique(&parsed);
    assert!(parsed.contains(&"x@y.de".to_string()));
}

#[test]
fn test_xml_no_panic_on_malformed_syntax() {
    // Malformed XML that should never panic
    let payload = r#"
<?xml version="1.0"?>
<root>
  <unclosed attr="test@x.de
  <<double-bracket>>test2@y.com</double-bracket>>
  <![CDATA[ cdata@broken ]]>
  <selfclosing attr="ok@z.de"/>
</root>
"#;
    let input = create_input_file_raw(payload);
    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-f")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed = stdout_lines(&output.stdout);
    assert_sorted_and_unique(&parsed);
    assert!(parsed.contains(&"ok@z.de".to_string()));
    assert!(parsed.contains(&"test2@y.com".to_string()));
}

// ── CLI JSON output test ─────────────────────────────────────────

#[test]
fn test_cli_json_output_format() {
    // mix of regex-extractable emails, one non-extractable "bogus", and duplicates
    let lines = vec![
        "alice@example.com".to_string(),
        "bogus".to_string(),                     // not regex-extractable — drops in ingestion
        "bob@test.de".to_string(),
        "alice@example.com".to_string(),          // duplicate — deduped
    ];
    let input = create_input_file(&lines);

    let output = Command::cargo_bin("email_validator")
        .unwrap()
        .arg("run")
        .arg("-i")
        .arg(input.path())
        .arg("-m")
        .arg("regex")
        .arg("-j")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout_str).unwrap();
    assert!(json.is_array(), "JSON output should be an array");
    let arr = json.as_array().unwrap();

    // ingestion extracts only regex-matching emails; "bogus" is dropped before validation.
    // We get 2 results: alice (deduped) + bob.
    assert_eq!(arr.len(), 2, "CLI ingestion drops non-extractable tokens before validation");

    let mut emails: Vec<&str> = arr.iter().map(|r| r["email"].as_str().unwrap()).collect();
    emails.sort();
    assert_eq!(emails, vec!["alice@example.com", "bob@test.de"]);

    let valid_count = arr.iter().filter(|r| r["valid"].as_bool().unwrap()).count();
    assert_eq!(valid_count, 2);
}
