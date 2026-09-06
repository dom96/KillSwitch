use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help_output() {
    // Automatically finds the binary named in your Cargo.toml
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

    cmd.arg("--help")
        .assert()
        .success() // Checks for exit code 0
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_cli_handles_errors() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

    cmd.arg("--bad-flag")
        .assert()
        .failure() // Checks for non-zero exit code
        .stderr(predicate::str::contains("error: unexpected argument"));
}

#[test]
fn test_cli_fahrenheit() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

    cmd.args(vec!["run", "tests/fahrenheit.ks"])
        .write_stdin("23\n")
        .assert()
        .success()
        .stdout(predicate::eq("73.4"));
}

#[test]
fn test_cli_factorial() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

    cmd.args(vec!["run", "tests/factorial.ks"])
        .write_stdin("11\n")
        .assert()
        .success()
        .stdout(predicate::eq("39916800"));
}

#[test]
fn test_cli_fizzbuzz() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

    let payload = include_bytes!("fizzbuzz_output.txt");

    cmd.args(vec!["run", "tests/fizzbuzz.ks"])
        .assert()
        .success()
        .stdout(predicate::eq(payload.as_slice()));
}

#[test]
fn test_cli_greeting() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

    cmd.args(vec!["run", "tests/greeting.ks"])
        .write_stdin("Alice\n32\n")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Hello")
                .and(predicate::str::contains("Alice"))
                .and(predicate::str::contains("32")),
        );
}

#[test]
fn test_cli_max() {
    let cmd = || Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

    cmd()
        .args(vec!["run", "tests/max.ks"])
        .write_stdin("2\n78847\n")
        .assert()
        .success()
        .stdout(predicate::eq("78847"));

    cmd()
        .args(vec!["run", "tests/max.ks"])
        .write_stdin("78847\n2\n")
        .assert()
        .success()
        .stdout(predicate::eq("78847"));

    cmd()
        .args(vec!["run", "tests/max.ks"])
        .write_stdin("-78847\n2\n")
        .assert()
        .success()
        .stdout(predicate::eq("2"));
}
