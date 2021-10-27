use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net;
use std::os::unix::fs::symlink;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use clap::{App, AppSettings, Arg};

fn main() -> Result<()> {
    let matches = App::new("x")
        .version("0.0.1")
        .about("swiss army knife for the command line")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::DisableHelpSubcommand)
        .subcommand(
            App::new("tcp")
                .about("TCP utilities")
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
            App::new("wal")
                .about("Logging utilities")
                .setting(AppSettings::DisableHelpSubcommand)
                .arg(
                    Arg::new("path")
                        .index(1)
                        .about("Path to write to")
                        .required(true),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("wal", matches)) => {
            let path: String = matches.value_of_t("path").unwrap();
            let path = std::path::Path::new(&path);
            do_wal(&path)?;
        }
        Some(("tcp", matches)) => {
            let port: u16 = matches.value_of_t("port").unwrap_or_else(|e| e.exit());
            let sock = net::SocketAddr::new(
                net::IpAddr::V4(net::Ipv4Addr::new(127, 0, 0, 1)),
                port,
            );
            match matches.subcommand_name() {
                Some("http") => do_http(sock)?,
                Some("merge") => do_merge(sock)?,
                Some("spread") => do_spread(sock)?,
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn do_http(sock: net::SocketAddr) -> Result<()> {
    let server = tiny_http::Server::http(sock).unwrap();
    for req in server.incoming_requests() {
        let res = tiny_http::Response::from_string("hello world\n".to_string());
        let _ = req.respond(res);
    }
    Ok(())
}

fn do_merge(sock: net::SocketAddr) -> Result<()> {
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

fn do_spread(sock: net::SocketAddr) -> Result<()> {
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

use glob::glob;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    use anyhow::Result;

    #[test]
    fn wal_bootstrap() -> Result<()> {
        let dir = tempdir()?;
        println!();
        println!("---");
        println!();
        println!("{:?}", dir);

        do_wal(dir.path())?;

        do_wal(dir.path())?;

        let output = std::process::Command::new("ls").arg("-al").output()?;
        io::stdout().write_all(&output.stdout).unwrap();

        println!();
        println!("---");
        println!();
        Ok(())
    }
}

fn do_wal(path: &std::path::Path) -> Result<()> {
    // TODO:
    // - max segment size as arg
    // - write stdin to segment
    // - rotate segment when max size reached
    // - tests
    fs::create_dir(path)
        .or_else(|e| match e.kind() {
            io::ErrorKind::AlreadyExists => Ok(()),
            _ => Err(e),
        })
        .with_context(|| format!("could not create directory `{}`", path.display()))?;

    std::env::set_current_dir(path)?;

    // let first = format!("{:020}", 0);
    // println!("first: {}", first);

    let mut expected = 0;
    let expr = "[0-9]".repeat(20);
    for segment in glob(&expr)?.map(|x| x.unwrap()) {
        let offset = segment
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(
            offset == expected,
            "expected: {:020}, have: {}",
            expected,
            segment.display(),
        );
        expected += segment.metadata().unwrap().len();
    }

    let current = format!("{:020}", expected);

    let mut fh = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&current)?;

    symlink(&current, "current").or_else(|e| match e.kind() {
        io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file("current");
            return symlink(current, "current");
        }
        _ => Err(e),
    })?;

    write!(fh, "hello\n")?;

    Ok(())
}
