use std::io::{self, BufRead, BufReader, Write};
use std::process;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;
use clap::{App, Arg, ArgMatches};

pub fn configure_app(app: App) -> App {
    return app
        .version("0.0.4")
        .about("Exec utilities")
        .arg(
            Arg::new("max-lines")
            .short('l')
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

type Downstream = Box<dyn Write + Send>;

fn run_exec(
    command: String,
    arguments: Vec<String>,
    max_lines: Option<u64>,
) -> Result<()> {
    let (tx, rx) = mpsc::sync_channel(0);
    thread::spawn(move || {
        let upstream = io::stdin();
        let buf = BufReader::new(upstream);
        let mut n = 0;

        let pull: Option<Downstream> = rx.recv().unwrap();
        let mut downstream = pull.unwrap();

        let mut lines = buf.lines();
        let mut line = lines.next();

        while line.is_some() {
            if writeln!(downstream, "{}", line.unwrap().unwrap()).is_err() {
                break;
            }
            downstream.flush().unwrap();

            n += 1;
            if let Some(m) = max_lines {
                if n >= m {
                    drop(downstream);
                    line = lines.next();
                    if line.is_none() {
                        break;
                    }
                    n = 0;
                    let pull = rx.recv().unwrap();
                    assert!(pull.is_none());
                    let pull = rx.recv().unwrap();
                    downstream = pull.unwrap();
                    continue;
                }
            }

            line = lines.next();
        }
    });

    loop {
        let status = spawn_child(&command, &arguments, &tx).unwrap();
        if let Err(_) = tx.send(None) {
            process::exit(status.code().unwrap());
        }
    }
}

fn spawn_child(
    command: &String,
    arguments: &Vec<String>,
    stdin: &mpsc::SyncSender<Option<Downstream>>,
) -> Result<process::ExitStatus> {
    let mut child = process::Command::new(command)
        .args(arguments)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("failed to execute process");

    {
        let downstream = child.stdin.take().unwrap();
        stdin.send(Some(Box::new(downstream))).unwrap();
    }

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
