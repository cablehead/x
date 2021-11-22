use std::io::{self, BufRead, BufReader, Write};
use std::process;
use std::thread;

use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use clap::{App, Arg, ArgMatches};

pub fn configure_app(app: App) -> App {
    return app
        .version("0.0.2")
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
    let command: String = matches.value_of_t("command").unwrap();
    let arguments = matches
        .values_of_t::<String>("arguments")
        .unwrap_or(Vec::new());
    run_exec(command, arguments)?;
    Ok(())
}

fn run_exec(command: String, arguments: Vec<String>) -> Result<()> {
    let mut child = process::Command::new(command)
        .args(arguments)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("failed to execute process");

    fn now() -> String {
        let now: DateTime<Utc> = Utc::now();
        return now.to_rfc3339_opts(SecondsFormat::Secs, true);
    }

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
        writeln!(&downstream, "{}:{}", now(), line).unwrap();
        downstream.flush().unwrap();
    }

    let status = child.wait().expect("failed to wait on child");
    process::exit(status.code().unwrap());
}
