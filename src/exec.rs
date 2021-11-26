use std::io::{self, BufRead, BufReader, Write};
use std::process;
use std::thread;

use anyhow::Result;
use clap::{App, Arg, ArgMatches};

pub fn configure_app(app: App) -> App {
    return app
        .version("0.0.3")
        .about("Exec utilities")
        .arg(
            Arg::new("max-lines")
            .long("max-lines")
            .about(
            "the number of lines to be sent to the exec'd process, before restarting it")
            .takes_value(true)
        )
        .arg(
            Arg::new("command")
                .index(1)
                .about("command to run")
                .required(true),
        )
        .arg(
            Arg::new("arguments")
                .index(2)
                .about("arguments")
                .multiple_values(true)
                .required(false),
        );
}

pub fn run(matches: &ArgMatches) -> Result<()> {
    let max_lines: Option<u64> = matches.value_of_t("max-lines").ok();
    let command: String = matches.value_of_t("command").unwrap();
    let arguments = matches
        .values_of_t::<String>("arguments")
        .unwrap_or(Vec::new());
    run_exec(command, arguments, max_lines)?;
    Ok(())
}

fn run_exec(
    command: String,
    arguments: Vec<String>,
    _max_lines: Option<u64>,
) -> Result<()> {
    let status = spawn_child(command, arguments).unwrap();
    process::exit(status.code().unwrap());
}

fn spawn_child(command: String, arguments: Vec<String>) -> Result<process::ExitStatus> {
    let mut child = process::Command::new(command)
        .args(arguments)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("failed to execute process");

    let upstream = io::stdin();
    let mut downstream = child.stdin.take().unwrap();
    thread::spawn(move || {
        let buf = BufReader::new(upstream);
        for line in buf.lines() {
            let line = line.unwrap();
            writeln!(&downstream, "{}", line).unwrap();
            downstream.flush().unwrap();
        }
    });

    let upstream = child.stdout.take().unwrap();
    let mut downstream = io::stdout();
    let buf = BufReader::new(upstream);
    for line in buf.lines() {
        let line = line.unwrap();
        writeln!(&downstream, "{}", line).unwrap();
        downstream.flush().unwrap();
    }

    Ok(child.wait().expect("failed to wait on child"))
}
