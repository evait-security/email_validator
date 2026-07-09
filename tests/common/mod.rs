#![allow(dead_code)]

use std::io::Write;
use tempfile::NamedTempFile;

pub fn create_input_file(lines: &[String]) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
    file
}

pub fn create_input_file_raw(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{content}").unwrap();
    file
}

pub fn build_mass_mixed_lines(total: usize) -> Vec<String> {
    let mut lines = Vec::with_capacity(total + 16);

    for i in 0..total {
        match i % 8 {
            0 => lines.push(format!("user{i}@example.com")),
            1 => lines.push(format!("zu.vor.nachnamen{i}@lol.das.test.de")),
            2 => lines.push(format!("broken{i}@")),
            3 => lines.push(format!("@missing-local{i}.de")),
            4 => lines.push(format!("noise-line-{i} no-email-here")),
            5 => lines.push(format!("<mail>xml.person{i}@a.b.c.example.net</mail>")),
            6 => lines.push(format!("[link](mailto:markdown.user{i}@docs.example.org)")),
            _ => lines.push(format!("duplicate{i}@dup.example.com duplicate{i}@dup.example.com")),
        }
    }

    lines.push("special.case@Deep.Mixed.Domain.Test.de".to_string());
    lines.push("special.case@Deep.Mixed.Domain.Test.de".to_string());
    lines.push("single.good@txt.test.de".to_string());
    lines.push("this is plain markdown text".to_string());
    lines.push("<contact email=\"xml.attribute@service.test.de\"/>".to_string());

    lines
}

// ── Markdown-specific mass line generator ──────────────────────────

pub fn build_mass_markdown_lines(total: usize) -> Vec<String> {
    let mut lines = Vec::with_capacity(total + 20);

    lines.push("# Email Contact List".to_string());
    lines.push("".to_string());
    lines.push("This document contains **various** email addresses for testing.".to_string());
    lines.push("".to_string());

    for i in 0..total {
        match i % 10 {
            0 => lines.push(format!("- Primary contact: `primary{i}@example.com`")),
            1 => lines.push(format!("- Backup contact: **backup{i}@sub.docs.test.de**")),
            2 => lines.push(format!("  - Nested list item with *nested{i}@deep.layer.example.org*")),
            3 => lines.push(format!("[Link to mail](mailto:support+team{i}@help.example.com)")),
            4 => lines.push(format!("```txt\ncode.block{i}@inline.codeblock.test.de\n```")),
            5 => lines.push(format!("## Section {i}  --  heading.{i}@section.title.example.net")),
            6 => lines.push(format!("| Col1 | Col2 |\n|------|------|\n| table{i}@row.example.com | broken{i}@ |")),
            7 => lines.push(format!("> Blockquote: blockquote{i}@quoted.example.org")),
            8 => lines.push(format!("~~strikethrough{i}@deleted.example.de~~ no-email")),
            _ => lines.push(format!("_italic{i}@italic.example.io_ and __bold{i}@bold.example.io__")),
        }
    }

    lines.push("".to_string());
    lines.push("---".to_string());
    lines.push("".to_string());
    lines.push("## Duplicates".to_string());
    lines.push("dup.markdown@example.com".to_string());
    lines.push("dup.markdown@example.com".to_string());
    lines.push("Dup.Markdown@Example.com".to_string());
    lines.push("".to_string());
    lines.push("## Invalid entries".to_string());
    lines.push("- `user@` (missing domain)".to_string());
    lines.push("- `@domain.tld` (missing local part)".to_string());
    lines.push("- `a..b@example.de` (double dot)".to_string());
    lines.push("- `name@domain.c` (single-char TLD)".to_string());
    lines.push("".to_string());
    lines.push("> **Note:** All addresses in this document are synthetic for testing.".to_string());

    lines
}

// ── XML-specific mass line generator ───────────────────────────────

pub fn build_mass_xml_lines(total: usize) -> Vec<String> {
    let mut lines = Vec::with_capacity(total + 20);

    lines.push(r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_string());
    lines.push(r#"<contacts>"#.to_string());
    lines.push(r#"  <meta generator="email_validator_test" version="1.0"/>"#.to_string());
    lines.push(format!("  <!-- {total} generated entries for fuzzing -->"));

    for i in 0..total {
        match i % 10 {
            0 => lines.push(format!(
                r#"  <entry email="user{i}@example.com" type="person">User {i}</entry>"#
            )),
            1 => lines.push(format!(
                r#"  <entry><name>Sub {i}</name><email>sub{i}@sub.docs.test.de</email></entry>"#
            )),
            2 => lines.push(format!(
                r#"  <entry type="alias"><![CDATA[cdata.person{i}@a.b.c.example.net]]></entry>"#
            )),
            3 => lines.push(format!(
                r#"  <note>Reachable at note+person{i}@notes.test.org during business hours</note>"#
            )),
            4 => lines.push(format!(
                r#"  <broken user="{i}">broken{i}@</broken>"#
            )),
            5 => lines.push(format!(
                r#"  <entry email="user.{i}@deep.nested.domain.test.de" role="admin"/>  <!-- self-closing -->"#
            )),
            6 => lines.push(format!(
                r#"  <invalid>@missing-local{i}.de</invalid>"#
            )),
            7 => lines.push(format!(
                r#"  <section><title>Group {i}</title><contact primary="group.primary{i}@groups.example.com" secondary="group.secondary{i}@groups.example.com"/></section>"#
            )),
            8 => lines.push(format!(
                r#"  <entry xmlns:em="urn:email"><em:addr>{i}.namespaced@ns.schema.test.org</em:addr></entry>"#
            )),
            _ => lines.push(format!(
                r#"  <raw>just-raw-text-{i} no-email-here</raw>"#
            )),
        }
    }

    lines.push(r#"  <duplicates>"#.to_string());
    lines.push(r#"    <dup email="dup.xml@example.com">First</dup>"#.to_string());
    lines.push(r#"    <dup email="dup.xml@example.com">Second</dup>"#.to_string());
    lines.push(r#"    <dup email="Dup.Xml@Example.com">Third (case variant)</dup>"#.to_string());
    lines.push(r#"  </duplicates>"#.to_string());
    lines.push(r#"</contacts>"#.to_string());

    lines
}

// ── Markdown string generator (flat, not line-based) ───────────────

pub fn build_markdown_string(total: usize) -> String {
    let mut md = String::new();

    md.push_str("# Email Contact List\n\n");
    md.push_str("This document contains **various** email addresses for testing.\n\n");

    for i in 0..total {
        match i % 10 {
            0 => md.push_str(&format!("- Primary: `primary{i}@example.com`\n")),
            1 => md.push_str(&format!("- Backup: **backup{i}@sub.docs.test.de**\n")),
            2 => md.push_str(&format!("  - Nested: *nested{i}@deep.layer.example.org*\n")),
            3 => md.push_str(&format!("[Mail link](mailto:support+team{i}@help.example.com)\n")),
            4 => md.push_str(&format!("```\ncode.block{i}@inline.codeblock.test.de\n```\n")),
            5 => md.push_str(&format!("## Section {i} / heading.{i}@section.title.example.net\n")),
            6 => md.push_str(&format!("| table{i}@row.example.com | broken{i}@ |\n")),
            7 => md.push_str(&format!("> blockquote{i}@quoted.example.org\n")),
            8 => md.push_str(&format!("~~strikethrough{i}@deleted.example.de~~\n")),
            _ => md.push_str(&format!("_italic{i}@italic.example.io_ __bold{i}@bold.example.io__\n")),
        }
    }

    md.push_str("\n---\n## Duplicates\n");
    md.push_str("dup.markdown@example.com\n");
    md.push_str("dup.markdown@example.com\n");
    md.push_str("Dup.Markdown@Example.com\n");
    md.push_str("\n## Invalid\n");
    md.push_str("- `user@`\n- `@domain.tld`\n- `a..b@example.de`\n- `name@domain.c`\n");

    md
}

// ── XML string generator (flat, not line-based) ────────────────────

pub fn build_xml_string(total: usize) -> String {
    let mut xml = String::new();

    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<contacts>"#);
    xml.push('\n');
    xml.push_str(&format!("  <!-- {total} generated entries -->\n"));

    for i in 0..total {
        match i % 10 {
            0 => xml.push_str(&format!(
                r#"  <entry email="user{i}@example.com" type="person">User {i}</entry>"#
            )),
            1 => xml.push_str(&format!(
                r#"  <entry><name>Sub {i}</name><email>sub{i}@sub.docs.test.de</email></entry>"#
            )),
            2 => xml.push_str(&format!(
                r#"  <entry type="alias"><![CDATA[cdata.person{i}@a.b.c.example.net]]></entry>"#
            )),
            3 => xml.push_str(&format!(
                r#"  <note>Reachable at note+person{i}@notes.test.org during business hours</note>"#
            )),
            4 => xml.push_str(&format!(
                r#"  <broken user="{i}">broken{i}@</broken>"#
            )),
            5 => xml.push_str(&format!(
                r#"  <entry email="user.{i}@deep.nested.domain.test.de" role="admin"/>"#
            )),
            6 => xml.push_str(&format!(
                r#"  <invalid>@missing-local{i}.de</invalid>"#
            )),
            7 => xml.push_str(&format!(
                r#"  <section><title>Group {i}</title><contact primary="group.primary{i}@groups.example.com" secondary="group.secondary{i}@groups.example.com"/></section>"#
            )),
            8 => xml.push_str(&format!(
                r#"  <entry xmlns:em="urn:email"><em:addr>{i}.namespaced@ns.schema.test.org</em:addr></entry>"#
            )),
            _ => xml.push_str(&format!(
                r#"  <raw>just-raw-text-{i} no-email-here</raw>"#
            )),
        }
        xml.push('\n');
    }

    xml.push_str(r#"  <duplicates>"#);
    xml.push('\n');
    xml.push_str(r#"    <dup email="dup.xml@example.com">First</dup>"#);
    xml.push('\n');
    xml.push_str(r#"    <dup email="dup.xml@example.com">Second</dup>"#);
    xml.push('\n');
    xml.push_str(r#"    <dup email="Dup.Xml@Example.com">Third</dup>"#);
    xml.push('\n');
    xml.push_str(r#"  </duplicates>"#);
    xml.push('\n');
    xml.push_str(r#"</contacts>"#);
    xml.push('\n');

    xml
}

pub fn assert_sorted_and_unique(values: &[String]) {
    let mut expected = values.to_vec();
    expected.sort();
    expected.dedup();
    assert_eq!(values, expected);
}
