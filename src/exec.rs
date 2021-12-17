use std::io::{self, BufRead, BufReader, Write};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
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

type Downstream = Box<dyn Write + Send>;

fn run_exec(
    command: String,
    arguments: Vec<String>,
    max_lines: Option<u64>,
) -> Result<()> {
    let done = Arc::new(AtomicBool::new(false));

    let (tx, rx) = mpsc::sync_channel(2);
    {
        let done = done.clone();
        thread::spawn(move || {
            let upstream = io::stdin();
            let buf = BufReader::new(upstream);
            let mut n = 0;
            let mut downstream: Downstream = rx.recv().unwrap();
            for line in buf.lines() {
                let line = line.unwrap();
                n += 1;
                if let Some(m) = max_lines {
                    if n > m {
                        drop(downstream);
                        downstream = rx.recv().unwrap();
                        n = 1;
                    }
                }
                writeln!(downstream, "{}", line).unwrap();
                downstream.flush().unwrap();
            }
            done.store(true, Ordering::Relaxed);
        });
    }

    loop {
        let status = spawn_child(&command, &arguments, &tx).unwrap();
        if done.load(Ordering::Relaxed) {
            process::exit(status.code().unwrap());
        }
    }
}

fn spawn_child(
    command: &String,
    arguments: &Vec<String>,
    stdin: &mpsc::SyncSender<Downstream>,
) -> Result<process::ExitStatus> {
    let mut child = process::Command::new(command)
        .args(arguments)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("failed to execute process");

    {
        let downstream = child.stdin.take().unwrap();
        let _ = stdin.send(Box::new(downstream));
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
