use anyhow::Result;
use clap::{App, AppSettings, Arg};

mod exec;
mod log;
mod stream;

fn main() -> Result<()> {
    let matches = App::new("x")
        .version("0.0.1")
        .about("Swiss army knife for the command line")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::DisableHelpSubcommand)
        .subcommand(
            App::new("stream")
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
                        .arg(Arg::new("follow").short('f').long("follow").about(
                            "wait for additional data to be appended to the log",
                        )),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("log", matches)) => log::run(matches)?,
        Some(("stream", matches)) => stream::run(matches)?,
        Some(("exec", matches)) => exec::run(matches)?,
        _ => unreachable!(),
    }

    Ok(())
}
