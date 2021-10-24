use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn tcp_args_port_required() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("x")?;
    cmd.arg("tcp");
    cmd.arg("merge");
    cmd.assert().failure().stderr(predicate::str::contains(
        "error: The following required arguments were not provided:\n    \
        --port <port>\n",
    ));
    Ok(())
}

#[test]
fn tcp_args_port_must_be_number() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("x")?;
    cmd.arg("tcp");
    cmd.args(["--port", "bar"]);
    cmd.arg("spread");
    cmd.assert().failure().stderr(predicate::str::contains(
        "The argument \'bar\' isn\'t a valid value for \'port\'",
    ));
    Ok(())
}
