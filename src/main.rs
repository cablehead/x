use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use clap::{App, AppSettings, Arg};

fn main() {
    let matches = App::new("x")
        .version("0.0.1")
        .about("swiss army knife for the command line")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .about("TCP port to listen on")
                .required(true)
                .takes_value(true),
        )
        .subcommand(
            App::new("spread")
                .about("Read lines from STDIN and writes them to all TCP connections"),
        )
        .subcommand(
            App::new("merge").about(
                "Read lines from TCP connections and writes them serially to STDOUT",
            ),
        )
        .get_matches();

    let port: u32 = matches.value_of_t("port").unwrap_or_else(|e| e.exit());
    match matches.subcommand_name() {
        Some("spread") => do_spread(port),
        Some("merge") => do_merge(port),
        _ => unreachable!(),
    }
}

fn do_spread(port: u32) {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

    let conns = Vec::new();
    let conns = Arc::new(Mutex::new(conns));

    {
        let conns = conns.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                let (tx, rx) = mpsc::channel();
                conns.lock().expect("poisoned").push(tx);
                thread::spawn(move || {
                    for line in rx {
                        if writeln!(&stream, "{}", line).is_err() {
                            break;
                        }
                    }
                });
            }
        });
    }

    let stdin = io::stdin();
    let buf = BufReader::new(stdin);
    for line in buf.lines() {
        let line = line.unwrap();
        conns
            .lock()
            .expect("poisoned")
            .retain(|conn| conn.send(line.clone()).is_ok());
    }
}

fn do_merge(port: u32) {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let tx = tx.clone();
            thread::spawn(move || {
                let buf = BufReader::new(&stream);
                for line in buf.lines() {
                    let line = line.unwrap();
                    tx.send(line).unwrap();
                }
            });
        }
    });

    let stdout = io::stdout();
    for line in rx {
        if writeln!(&stdout, "{}", line).is_err() {
            break;
        }
    }
}
