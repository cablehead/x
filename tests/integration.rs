use assert_cmd::prelude::*;
use predicates::prelude::*;

use std::io::Read;
use std::process::{Command, Stdio};

#[test]
fn stream_args_port_required() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("x")?;
    cmd.arg("stream");
    cmd.arg("merge");
    cmd.assert().failure().stderr(predicate::str::contains(
        "error: The following required arguments were not provided:\n    \
        --port <port>\n",
    ));
    Ok(())
}

#[test]
fn stream_args_port_must_be_number() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("x")?;
    cmd.arg("stream");
    cmd.args(["--port", "bar"]);
    cmd.arg("broadcast");
    cmd.assert().failure().stderr(predicate::str::contains(
        "The argument \'bar\' isn\'t a valid value for \'port\'",
    ));
    Ok(())
}

#[test]
fn exec() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("x")?
        .arg("exec")
        .arg("--")
        .arg("echo")
        .arg("test")
        .arg("1-2-3")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdout = cmd.stdout.take().unwrap();
    let mut got = String::new();
    stdout.read_to_string(&mut got)?;
    assert_eq!("test 1-2-3\n", got);
    Ok(())
}
