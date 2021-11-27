use assert_cmd::prelude::*;
use predicates::prelude::*;

use std::io::{Read, Write};
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
fn exec_out() -> Result<(), Box<dyn std::error::Error>> {
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
    assert!(cmd.wait()?.success());
    Ok(())
}

#[test]
fn exec_in_out() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("x")?
        .arg("exec")
        .arg("--")
        .arg("wc")
        .arg("-l")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    {
        let stdin = cmd.stdin.take().unwrap();
        for _ in 0..10 {
            writeln!(&stdin, "hi").unwrap();
        }
    }
    let mut stdout = cmd.stdout.take().unwrap();

    let mut got = String::new();
    stdout.read_to_string(&mut got)?;
    assert_eq!("10\n", got.trim_start());
    assert!(cmd.wait()?.success());
    Ok(())
}

#[test]
fn exec_max_lines() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("x")?
        .arg("exec")
        .arg("--max-lines")
        .arg("2")
        .arg("--")
        .arg("wc")
        .arg("-l")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    {
        let stdin = cmd.stdin.take().unwrap();
        for _ in 0..10 {
            writeln!(&stdin, "hi").unwrap();
        }
    }
    let mut stdout = cmd.stdout.take().unwrap();

    let mut got = String::new();
    stdout.read_to_string(&mut got)?;
    got.retain(|c| c != ' ');
    assert_eq!("2\n2\n2\n2\n2\n", got);
    assert!(cmd.wait()?.success());
    Ok(())
}
