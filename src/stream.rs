use std::io::{self, BufRead, BufReader, Write};
use std::net;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches};
use serde_json::json;

pub fn configure_app(app: App) -> App {
    return app
        .version("0.0.3")
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
                "Read lines from TCP connections and write them serially to STDOUT",
            ),
        )
        .subcommand(
            App::new("broadcast")
                .about("Read lines from STDIN and write them to all TCP connections")
                .arg(
                    Arg::new("history")
                        .short('i')
                        .long("history")
                        .about("number of lines to keep in memory to be sent immediately to new connections")
                        .takes_value(true)
                        .default_value("0"),
                ),
        );
}

pub fn run(matches: &ArgMatches) -> Result<()> {
    let port: u16 = matches.value_of_t("port").unwrap_or_else(|e| e.exit());
    let sock =
        net::SocketAddr::new(net::IpAddr::V4(net::Ipv4Addr::new(127, 0, 0, 1)), port);
    match matches.subcommand() {
        Some(("http", _)) => run_http(sock)?,
        Some(("merge", _)) => run_merge(sock)?,
        Some(("broadcast", matches)) => {
            let history: usize =
                matches.value_of_t("history").unwrap_or_else(|e| e.exit());
            run_broadcast(sock, history)?
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn run_http(sock: net::SocketAddr) -> Result<()> {
    let server = tiny_http::Server::http(sock).unwrap();
    for mut req in server.incoming_requests() {
        thread::spawn(move || {
            // todo RequestID  uuid.UUID   `json:"request_id"`

            let mut buffer = String::new();
            req.as_reader().read_to_string(&mut buffer).unwrap();
            let b64 = base64::encode_config(buffer, base64::URL_SAFE);

            // gosh, this is terrible. I need to get better with rust's type system
            let headers: Vec<(String, String)> = req
                .headers()
                .iter()
                .map(|x| (format!("{}", x.field.as_str()), format!("{}", x.value)))
                .collect();

            let packet = json!({
                "topic": "http.request",
                "content": {
                    "method": req.method().as_str(),
                    "headers": headers,
                    "remote_addr": req.remote_addr(),
                    "url": req.url(),
                    "body": b64,
                },
            });
            println!("{}", packet);

            let res = tiny_http::Response::from_string("hello world\n".to_string());
            let _ = req.respond(res);
        });
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

fn run_broadcast(sock: net::SocketAddr, history: usize) -> Result<()> {
    let listener = net::TcpListener::bind(sock).unwrap();

    let conns = Vec::new();
    let conns = Arc::new(Mutex::new(conns));

    let buffer = Vec::new();
    let buffer = Arc::new(Mutex::new(buffer));

    {
        let conns = conns.clone();
        let buffer = buffer.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                if history > 0 {
                    let buffer = buffer.lock().expect("poisoned");
                    let mut is_err = false;
                    for line in buffer.iter() {
                        if writeln!(&stream, "{}", line).is_err() {
                            is_err = true;
                            break;
                        }
                    }
                    if is_err {
                        continue;
                    }
                }
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
        if history > 0 {
            let mut buffer = buffer.lock().expect("poisoned");
            buffer.push(line);
            if buffer.len() > history {
                buffer.remove(0);
            }
        }
    }
    Ok(())
}
