use std::io::{self, BufRead, BufReader, Write};
use std::net;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches};

pub fn configure_app(app: App) -> App {
    return app
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
        .subcommand(
            App::new("merge").about(
                "Read lines from TCP connections and writes them serially to STDOUT",
            ),
        )
        .subcommand(
            App::new("spread")
                .about("Read lines from STDIN and writes them to all TCP connections"),
        );
}

pub fn run(matches: &ArgMatches) -> Result<()> {
    let port: u16 = matches.value_of_t("port").unwrap_or_else(|e| e.exit());
    let sock =
        net::SocketAddr::new(net::IpAddr::V4(net::Ipv4Addr::new(127, 0, 0, 1)), port);
    match matches.subcommand_name() {
        Some("http") => run_http(sock)?,
        Some("merge") => run_merge(sock)?,
        Some("spread") => run_spread(sock)?,
        _ => unreachable!(),
    }
    Ok(())
}

fn run_http(sock: net::SocketAddr) -> Result<()> {
    let server = tiny_http::Server::http(sock).unwrap();
    for req in server.incoming_requests() {
        let res = tiny_http::Response::from_string("hello world\n".to_string());
        let _ = req.respond(res);
    }
    Ok(())
}

fn run_merge(sock: net::SocketAddr) -> Result<()> {
    let listener = net::TcpListener::bind(sock).unwrap();

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
    Ok(())
}

fn run_spread(sock: net::SocketAddr) -> Result<()> {
    let listener = net::TcpListener::bind(sock).unwrap();

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
    Ok(())
}
