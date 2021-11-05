use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, Write};
use std::net;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use clap::{App, AppSettings, Arg};

fn main() -> Result<()> {
    let matches = App::new("x")
        .version("0.0.1")
        .about("Swiss army knife for the command line")
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
                    App::new("read").about("read from the log to STDOUT").arg(
                        Arg::new("cursor")
                            .short('c')
                            .long("cursor")
                            .about("current cursor to read from")
                            .default_value("0")
                            .takes_value(true),
                    ),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("log", matches)) => {
            let path: String = matches.value_of_t("path").unwrap();
            let path = Path::new(&path);
            match matches.subcommand() {
                Some(("write", matches)) => {
                    let max_segment: u64 = matches.value_of_t("max-segment").unwrap();
                    do_log_write(io::stdin(), &path, max_segment * 1024 * 1024)?;
                }
                Some(("read", matches)) => {
                    let cursor: u64 = matches.value_of_t("cursor").unwrap();
                    do_log_read(&mut io::stdout(), &path, cursor)?;
                }
                _ => unreachable!(),
            }
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
        Some(("exec", matches)) => {
            let command: String = matches.value_of_t("command").unwrap();
            let arguments = matches.values_of_t::<String>("arguments").unwrap();
            do_exec(command, arguments)?;
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
    use super::{do_log_read, do_log_write};

    use std::io::{self, Read, Write};
    use std::str::from_utf8;

    use anyhow::Result;
    use tempfile::tempdir;

    #[test]
    fn log_bootstrap() -> Result<()> {
        // TODO: assert the state of the files after two calls to do_log_write
        let dir = tempdir()?;
        let path = dir.path();

        println!();
        println!("---");
        println!();
        println!("DIR {:?}", path);

        fn stdin() -> impl Read {
            io::Cursor::new(
                format!("{}\n", "x".repeat(1024))
                    .repeat(4608)
                    .as_bytes()
                    .to_vec(),
            )
        }

        do_log_write(stdin(), path, 1024 * 1024)?;
        let output = std::process::Command::new("ls")
            .current_dir(path)
            .arg("-alh")
            .output()?;
        io::stdout().write_all(&output.stdout).unwrap();

        do_log_write(stdin(), path, 1024 * 1024)?;
        let output = std::process::Command::new("ls")
            .current_dir(path)
            .arg("-alh")
            .output()?;
        io::stdout().write_all(&output.stdout).unwrap();

        println!();
        println!("---");
        println!();
        Ok(())
    }

    #[test]
    fn log_read() -> Result<()> {
        let dir = tempdir()?;
        let path = dir.path();
        println!("DIR {:?}", path);

        let segment1 = "one\ntwo\nthree\nfour\n";
        let segment2 = "one-2\ntwo-2\nthree-2\nfour-2\n";
        let segment3 = "one-3\ntwo-3\nthree-3\nfour-3\n";

        // write the first segment
        do_log_write(io::Cursor::new(segment1), path, 1024 * 1024)?;

        // read all
        let mut stdout = io::Cursor::new(Vec::new());
        do_log_read(&mut stdout, path, 0)?;
        assert_eq!(from_utf8(stdout.get_ref())?, segment1);

        // read from cursor
        let mut stdout = io::Cursor::new(Vec::new());
        do_log_read(&mut stdout, path, "one\n".len() as u64)?;
        assert_eq!(from_utf8(stdout.get_ref())?, "two\nthree\nfour\n");

        // write again to generate two more segments
        do_log_write(io::Cursor::new(segment2), path, 1024 * 1024)?;
        do_log_write(io::Cursor::new(segment3), path, 1024 * 1024)?;

        // read all
        let mut stdout = io::Cursor::new(Vec::new());
        do_log_read(&mut stdout, path, 0)?;
        assert_eq!(
            from_utf8(stdout.get_ref())?,
            [segment1, segment2, segment3].join("")
        );

        // read from cursor that points into the second segment
        let mut stdout = io::Cursor::new(Vec::new());
        do_log_read(&mut stdout, path, (segment1.len() + "one-2\n".len()) as u64)?;
        assert_eq!(
            from_utf8(stdout.get_ref())?,
            ["two-2\nthree-2\nfour-2\n", segment3].join("")
        );

        // read from cursor that points into the third segment
        let mut stdout = io::Cursor::new(Vec::new());
        do_log_read(
            &mut stdout,
            path,
            (segment1.len() + segment2.len() + "one-3\n".len()) as u64,
        )?;
        assert_eq!(from_utf8(stdout.get_ref())?, "two-3\nthree-3\nfour-3\n");

        Ok(())
    }
}

fn do_log_read<W: Write>(w: &mut W, path: &Path, cursor: u64) -> Result<()> {
    let mut expected = 0;

    let expr = path.join("[0-9]".repeat(20));
    let expr = expr.to_str().unwrap();

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

        let current = expected;
        expected += segment.metadata().unwrap().len();

        // fast forward until we find the segment our cursor is in
        if cursor >= expected {
            continue;
        }

        let mut fh = fs::OpenOptions::new().read(true).open(&segment)?;
        // fast forward within the current segment
        if cursor > current {
            fh.seek(io::SeekFrom::Start(cursor - current)).unwrap();
        }

        let buf = BufReader::new(fh);
        for line in buf.lines() {
            let line = line.unwrap();
            writeln!(w, "{}", &line).unwrap();
        }
    }

    Ok(())
}

fn do_log_write<R: Read>(r: R, path: &Path, max_segment: u64) -> Result<()> {
    fs::create_dir(path)
        .or_else(|e| match e.kind() {
            io::ErrorKind::AlreadyExists => Ok(()),
            _ => Err(e),
        })
        .with_context(|| format!("could not create directory `{}`", path.display()))?;

    let mut expected = 0;

    let expr = path.join("[0-9]".repeat(20));
    let expr = expr.to_str().unwrap();

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

    fn open_current(path: &Path, expected: u64) -> Result<fs::File> {
        let current = format!("{:020}", expected);
        let fh = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path.join(&current))?;

        let link = path.join("current");
        symlink(&current, &link).or_else(|e| match e.kind() {
            io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&link);
                return symlink(&current, &link);
            }
            _ => Err(e),
        })?;

        Ok(fh)
    }

    let mut fh = open_current(path, expected)?;
    let mut fh_size = fh.metadata()?.len();

    let buf = BufReader::new(r);
    for line in buf.lines() {
        let line = line.unwrap();

        let new_bytes = line.len() as u64 + 1;

        assert!(
            new_bytes <= max_segment,
            "max_segment = {}, new_bytes = {}",
            max_segment,
            new_bytes
        );

        if fh_size + new_bytes > max_segment {
            expected += fh_size;
            fh = open_current(path, expected)?;
            fh_size = 0;
        }

        writeln!(fh, "{}", &line).unwrap();
        fh_size += new_bytes;
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
