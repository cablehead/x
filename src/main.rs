use std::io::{self, BufRead, BufReader, Write};
use std::process;
use std::thread;

use anyhow::Result;
use clap::{App, AppSettings, Arg};

mod log;
mod stream;

fn main() -> Result<()> {
    let matches = App::new("x")
        .version("0.0.1")
        .about("Swiss army knife for the command line")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::DisableHelpSubcommand)
        .subcommand(
            App::new("stream")
                .about("Network utilities")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .setting(AppSettings::DisableHelpSubcommand)
                .arg(
                    Arg::new("port")
                        .short('p')
                        .long("port")
                        .about("TCP port to listen on")
                        .required(true)
                        .takes_value(true),
                )
                .subcommand(App::new("http").about(
                    "Serve HTTP. Requests are written to STDOUT and \
                    responses are read from STDIN",
                ))
                .subcommand(App::new("merge").about(
                    "Read lines from TCP connections and writes them serially to STDOUT",
                ))
                .subcommand(App::new("spread").about(
                    "Read lines from STDIN and writes them to all TCP connections",
                )),
        )
        .subcommand(
            App::new("exec")
                .about("Exec utilities")
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
                ),
        )
        .subcommand(
            App::new("log")
                .about("Logging utilities")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .setting(AppSettings::DisableHelpSubcommand)
                .arg(
                    Arg::new("path")
                        .index(1)
                        .about("Path to write to")
                        .required(true),
                )
                .subcommand(
                    App::new("write").about("write STDIN to the log").arg(
                        Arg::new("max-segment")
                            .short('m')
                            .long("max-segment")
                            .about("maximum size for each segment in MB")
                            .default_value("100")
                            .takes_value(true),
                    ),
                )
                .subcommand(
                    App::new("read")
                        .about("read from the log to STDOUT")
                        .arg(
                            Arg::new("cursor")
                                .short('c')
                                .long("cursor")
                                .about("current cursor to read from")
                                .default_value("0")
                                .takes_value(true),
                        )
                        .arg(Arg::new("follow").short('f').long("follow").about(
                            "wait for additional data to be appended to the log",
                        )),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("log", matches)) => log::run(matches)?,
        Some(("stream", matches)) => stream::run(matches)?,
        Some(("exec", matches)) => {
            let command: String = matches.value_of_t("command").unwrap();
            let arguments = matches.values_of_t::<String>("arguments").unwrap();
            do_exec(command, arguments)?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

use chrono::{DateTime, SecondsFormat, Utc};

fn do_exec(command: String, arguments: Vec<String>) -> Result<()> {
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
