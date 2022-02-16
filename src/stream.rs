use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::Result;
use clap::{Command, Arg, ArgMatches};
use serde::{Deserialize, Serialize};
use serde_json;
use uuid::Uuid;

pub fn configure_app(app: Command) -> Command {
    return app
        .version("0.0.3")
        .about("Network utilities")
        .subcommand_required(true)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .help("TCP port to listen on")
                .required(true)
                .takes_value(true),
        )
        .subcommand(Command::new("http").about(
            "Serve HTTP. Requests are written to STDOUT and \
                    responses are read from STDIN",
        ))
        .subcommand(
            Command::new("merge").about(
                "Read lines from TCP connections and write them serially to STDOUT",
            ),
        )
        .subcommand(
            Command::new("broadcast")
                .about("Read lines from STDIN and write them to all TCP connections")
                .arg(
                    Arg::new("history")
                        .short('i')
                        .long("history")
                        .help("number of lines to keep in memory to be sent immediately to new connections")
                        .takes_value(true)
                        .default_value("0"),
                ),
        );
}

pub fn run(matches: &ArgMatches) -> Result<()> {
    let port: u16 = matches.value_of_t("port").unwrap_or_else(|e| e.exit());
    let sock =
        net::SocketAddr::new(net::IpAddr::V4(net::Ipv4Addr::new(0, 0, 0, 0)), port);
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
    #[derive(Serialize, Deserialize)]
    struct Response {
        request_id: String,
        body: String,
        headers: Option<Vec<(String, String)>>,
    }

    let requests: HashMap<String, mpsc::Sender<Response>> = HashMap::new();
    let requests = Arc::new(Mutex::new(requests));

    {
        let requests = requests.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            let buf = BufReader::new(stdin);
            for line in buf.lines() {
                let line = line.unwrap();

                let res = serde_json::from_str(&line);
                if res.is_err() {
                    println!(
                        "{}",
                        serde_json::json!({
                            "topic": "http.response.log",
                            "content": line,
                            "severity": "ERROR",
                            "error": "unable to parse response",
                        })
                    );
                    continue;
                }
                let res: Response = res.unwrap();

                let mut requests = requests.lock().expect("poisoned");
                if let Some(tx) = requests.remove(&res.request_id) {
                    tx.send(res).unwrap();
                } else {
                    println!(
                        "{}",
                        serde_json::json!({
                            "topic": "http.response.log",
                            "content": res,
                            "severity": "ERROR",
                            "error": "unknown request_id",
                        })
                    );
                }
            }
        });
    }

    let server = tiny_http::Server::http(sock).unwrap();
    for mut req in server.incoming_requests() {
        let requests = requests.clone();
        thread::spawn(move || {
            let uid = Uuid::new_v4();

            let mut buffer = String::new();
            req.as_reader().read_to_string(&mut buffer).unwrap();
            let b64 = base64::encode_config(buffer, base64::URL_SAFE);

            // gosh, this is terrible. I need to get better with rust's type system
            let headers: Vec<(String, String)> = req
                .headers()
                .iter()
                .map(|x| (format!("{}", x.field.as_str()), format!("{}", x.value)))
                .collect();

            let packet = serde_json::json!({
                "topic": "http.request",
                "content": {
                    "method": req.method().as_str(),
                    "headers": headers,
                    "remote_addr": req.remote_addr(),
                    "url": req.url(),
                    "body": b64,
                    "request_id": uid,
                },
            });

            let (tx, rx) = mpsc::channel();

            {
                let mut requests = requests.lock().expect("poisoned");
                requests.insert(uid.to_string(), tx);
            }
            println!("{}", packet);

            let res = rx.recv().unwrap();

            let mut http_response = tiny_http::Response::from_string(&res.body);

            let header = tiny_http::Header::from_bytes(
                &b"Content-Type"[..],
                &b"text/html; charset=utf8"[..],
            )
            .unwrap();
            http_response = http_response.with_header(header);

            if let Some(ref headers) = res.headers {
                for header in headers {
                    let (key, value) = header;
                    let add = tiny_http::Header::from_bytes(key.clone(), value.clone()).unwrap();
                    http_response = http_response.with_header(add);
                }
            }

            let _ = req.respond(http_response);

            println!(
                "{}",
                serde_json::json!({
                    "topic": "http.response.log",
                    "content": res,
                    "severity": "INFO",
                })
            );
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
