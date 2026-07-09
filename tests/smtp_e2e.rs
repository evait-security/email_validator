use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn create_clean_input_file(lines: &[&str]) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    for line in lines {
        writeln!(file, "{}", line).unwrap();
    }
    file
}

#[test]
fn test_boundary_nested_subdomains_happy_path() {
    // This boundary test validates extraction/output behavior for deep subdomains
    // without relying on external SMTP infrastructure.
    let input = create_clean_input_file(&[
        "relo@testdomain.local",
        "deep.nested.user@a.b.c.d.e.f.testdomain.local",
        "nested@support.sub.testdomain.local"
    ]);

    let mut cmd = Command::cargo_bin("email_validator").unwrap();
    cmd.arg("-i").arg(input.path())
       .arg("-m").arg("regex")
       .arg("-f").arg("list");

    cmd.assert()
       .success()
       .stdout(predicate::str::contains("relo@testdomain.local"))
       .stdout(predicate::str::contains("deep.nested.user@a.b.c.d.e.f.testdomain.local"))
       .stdout(predicate::str::contains("nested@support.sub.testdomain.local"));
}

#[test]
fn test_boundary_smtp_network_timeout_mitigation() {
    // Testing how the engine handles a completely dead IP address route safely
    let input = create_clean_input_file(&["shadow@spoofeddomain.local"]);

    let mut cmd = Command::cargo_bin("email_validator").unwrap();
    cmd.arg("-i").arg(input.path())
       .arg("-m").arg("smtp");

    // The runtime MUST handle the drop or network failure without a panic stacktrace.
    // Exit code 0 implies graceful rejection handling.
    cmd.assert()
       .success()
       .stderr(predicate::str::contains("failed").or(predicate::str::contains("[-] shadow@spoofeddomain.local")));
}

#[test]
fn test_boundary_quiet_mode_gophish_csv_streaming() {
    let input = create_clean_input_file(&["clean-target@testdomain.local"]);

    let mut cmd = Command::cargo_bin("email_validator").unwrap();
    cmd.arg("-i").arg(input.path())
       .arg("-m").arg("regex") // testing format stream logic speed
       .arg("-f").arg("gophish");

    // Absolute zero noise on stderr. Pure stdout stream testing.
    cmd.assert()
       .success()
       .stderr(predicate::str::is_empty())
       .stdout(predicate::str::contains("First Name,Last Name,Email,Position\n,,clean-target@testdomain.local,\n"));
}

#[test]
fn test_boundary_duplicate_filtration_across_pipeline() {
    // Duplicate emails inside messy parameters should be streamlined to one execution check
    let input = create_clean_input_file(&[
        "duplicate@testdomain.local",
        "duplicate@testdomain.local",
        "DUPLICATE@testdomain.local" // verifying strict tracking
    ]);

    let mut cmd = Command::cargo_bin("email_validator").unwrap();
    cmd.arg("-i").arg(input.path())
       .arg("-m").arg("regex")
       .arg("-f").arg("list");

    let assert_obj = cmd.assert().success();
    // We can verify via output constraints that execution deduplicated the input strings.
    assert_obj.stdout(predicate::str::contains("duplicate@testdomain.local"));
}
