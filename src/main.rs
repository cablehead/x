/*
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
*/

use clap::{App, Arg}; //, SubCommand};

fn main() {
    let matches = App::new("x")
        .version("0.0.1")
        .about("swiss army knife for the command line")
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .about("TCP port to listen on")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let p: u32 = matches.value_of_t("port").unwrap_or_else(|e| e.exit());
    println!("{:?}", p);
}

/*
fn do_spread() {
    let listener = TcpListener::bind("127.0.0.1:7879").unwrap();

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

fn do_merge() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

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
*/
