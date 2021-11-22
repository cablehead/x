use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, Write};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::thread;
use std::time;

use anyhow::{Context, Result};
use clap::{App, AppSettings, Arg, ArgMatches};
use glob::glob;

pub fn configure_app(app: App) -> App {
    return app
        .version("0.0.3")
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
                .arg(
                    Arg::new("follow")
                        .short('f')
                        .long("follow")
                        .about("wait for additional data to be appended to the log"),
                )
                .arg(Arg::new("track").short('t').long("track").about(
                    "write the cursor of each line read to STDERR so clients \
                            can resume reads",
                ))
                .subcommand(
                    App::new("exec")
                        .about(
                            "execute a command for each line read from the log. \
                            If the command exits with a 0 / successful error code, \
                            the cursor of the read line is written to STDERR. \
                            Otherwise the read terminates emitting the same \
                            error code.",
                        )
                        .setting(AppSettings::DisableHelpSubcommand)
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
                ),
        );
}

pub fn run(matches: &ArgMatches) -> Result<()> {
    let path: String = matches.value_of_t("path").unwrap();
    let path = Path::new(&path);
    match matches.subcommand() {
        Some(("write", matches)) => {
            let max_segment: u64 = matches.value_of_t("max-segment").unwrap();
            run_write(io::stdin(), &path, max_segment * 1024 * 1024)?;
        }
        Some(("read", matches)) => {
            let cursor: u64 = matches.value_of_t("cursor").unwrap();
            let follow: bool = matches.is_present("follow");

            let mut stderr = io::stderr();
            let track = if matches.is_present("track") {
                Some(&mut stderr)
            } else {
                None
            };

            run_read(&mut io::stdout(), &path, cursor, follow, track)?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn run_write<R: Read>(r: R, path: &Path, max_segment: u64) -> Result<()> {
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

fn run_read<W: Write, T: Write>(
    w: &mut W,
    path: &Path,
    cursor: u64,
    follow: bool,
    mut track: Option<&mut T>,
) -> Result<()> {
    let mut offset = 0;

    loop {
        let segment = path.join(format!("{:020}", offset));
        let segment_size = segment.metadata().unwrap().len();

        // fast forward until we find the segment our cursor is in
        if cursor > offset + segment_size {
            offset += segment_size;
            continue;
        }

        let mut fh = fs::OpenOptions::new().read(true).open(&segment)?;
        // fast forward within the current segment
        if cursor > offset {
            fh.seek(io::SeekFrom::Start(cursor - offset)).unwrap();
            offset = cursor;
        }

        let mut lines = BufReader::new(&fh).lines();
        loop {
            match lines.next() {
                Some(line) => {
                    let line = line.unwrap();
                    writeln!(w, "{}", &line).unwrap();
                    offset += line.len() as u64 + 1;
                    if let Some(ref mut t) = track {
                        writeln!(t, "{}", offset).unwrap();
                    }
                }
                None => {
                    // is the next segment available?
                    let next_segment = path.join(format!("{:020}", offset));
                    if next_segment.is_file() {
                        break;
                    }

                    if !follow {
                        return Ok(());
                    }

                    // poll the current segment for new data
                    let m = time::Duration::from_millis(50);
                    thread::sleep(m);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{run_read, run_write};

    use std::fs;
    use std::io::{self, Read, Write};
    use std::str::from_utf8;

    use anyhow::Result;
    use tempfile::tempdir;

    #[test]
    fn log_bootstrap() -> Result<()> {
        // TODO: assert the state of the files after two calls to run_write
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

        run_write(stdin(), path, 1024 * 1024)?;
        let output = std::process::Command::new("ls")
            .current_dir(path)
            .arg("-alh")
            .output()?;
        io::stdout().write_all(&output.stdout).unwrap();

        run_write(stdin(), path, 1024 * 1024)?;
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
        run_write(io::Cursor::new(segment1), path, 1024 * 1024)?;

        // read all
        let mut stdout = io::Cursor::new(Vec::new());
        run_read(&mut stdout, path, 0, false, None::<&mut fs::File>)?;
        assert_eq!(from_utf8(stdout.get_ref())?, segment1);

        // read from cursor
        let mut stdout = io::Cursor::new(Vec::new());
        run_read(
            &mut stdout,
            path,
            "one\n".len() as u64,
            false,
            None::<&mut fs::File>,
        )?;
        assert_eq!(from_utf8(stdout.get_ref())?, "two\nthree\nfour\n");

        // write again to generate two more segments
        run_write(io::Cursor::new(segment2), path, 1024 * 1024)?;
        run_write(io::Cursor::new(segment3), path, 1024 * 1024)?;

        // read all
        let mut stdout = io::Cursor::new(Vec::new());
        run_read(&mut stdout, path, 0, false, None::<&mut fs::File>)?;
        assert_eq!(
            from_utf8(stdout.get_ref())?,
            [segment1, segment2, segment3].join("")
        );

        // read from cursor that points into the second segment
        let mut stdout = io::Cursor::new(Vec::new());
        run_read(
            &mut stdout,
            path,
            (segment1.len() + "one-2\n".len()) as u64,
            false,
            None::<&mut fs::File>,
        )?;
        assert_eq!(
            from_utf8(stdout.get_ref())?,
            ["two-2\nthree-2\nfour-2\n", segment3].join("")
        );

        // read from cursor that points into the third segment
        let mut stdout = io::Cursor::new(Vec::new());
        run_read(
            &mut stdout,
            path,
            (segment1.len() + segment2.len() + "one-3\n".len()) as u64,
            false,
            None::<&mut fs::File>,
        )?;
        assert_eq!(from_utf8(stdout.get_ref())?, "two-3\nthree-3\nfour-3\n");

        // read from cursor that points to the end of the third segment
        let mut stdout = io::Cursor::new(Vec::new());
        run_read(
            &mut stdout,
            path,
            (segment1.len() + segment2.len() + segment3.len()) as u64,
            false,
            None::<&mut fs::File>,
        )?;
        assert_eq!(from_utf8(stdout.get_ref())?, "");

        Ok(())
    }
}
