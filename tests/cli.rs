use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn lockstep() -> Command {
    Command::cargo_bin("lockstep").unwrap()
}

#[test]
fn help_lists_subcommands() {
    lockstep().arg("--help").assert().success().stdout(
        predicate::str::contains("run")
            .and(predicate::str::contains("check"))
            .and(predicate::str::contains("list")),
    );
}

#[test]
fn list_reports_ecosystems_on_empty_repo() {
    let dir = tempdir().unwrap();
    lockstep()
        .args(["list", "--path"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("uv").and(predicate::str::contains("detected=false")));
}

#[test]
fn check_writes_nothing_on_empty_repo() {
    let dir = tempdir().unwrap();
    lockstep()
        .args(["check", "--path"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("keeping 'em in step"));
}

#[test]
fn completions_emit_bash() {
    lockstep()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("lockstep"));
}

#[test]
fn rejects_unknown_freeze_key() {
    lockstep()
        .args(["run", "--freeze", "cargo"])
        .assert()
        .failure();
}
