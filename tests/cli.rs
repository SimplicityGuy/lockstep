use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn clockpin() -> Command {
    Command::cargo_bin("clockpin").unwrap()
}

#[test]
fn help_lists_subcommands() {
    clockpin().arg("--help").assert().success().stdout(
        predicate::str::contains("run")
            .and(predicate::str::contains("check"))
            .and(predicate::str::contains("list")),
    );
}

#[test]
fn list_reports_ecosystems_on_empty_repo() {
    let dir = tempdir().unwrap();
    clockpin()
        .args(["list", "--path"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("uv").and(predicate::str::contains("detected=false")));
}

#[test]
fn check_writes_nothing_on_empty_repo() {
    let dir = tempdir().unwrap();
    clockpin()
        .args(["check", "--path"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("continuous lockfile pin updates"));
}

#[test]
fn completions_emit_bash() {
    clockpin()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("clockpin"));
}

#[test]
fn rejects_unknown_freeze_key() {
    clockpin()
        .args(["run", "--freeze", "cargo"])
        .assert()
        .failure();
}
