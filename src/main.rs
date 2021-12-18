use anyhow::Result;
use clap::{App, AppSettings};

mod exec;
mod log;
mod stream;

fn main() -> Result<()> {
    let matches = App::new("x")
        .version("0.0.4")
        .about("Swiss army knife for the command line")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::DisableHelpSubcommand)
        .subcommand(exec::configure_app(App::new("exec")))
        .subcommand(log::configure_app(App::new("log")))
        .subcommand(stream::configure_app(App::new("stream")))
        .get_matches();

    match matches.subcommand() {
        Some(("exec", matches)) => exec::run(matches)?,
        Some(("log", matches)) => log::run(matches)?,
        Some(("stream", matches)) => stream::run(matches)?,
        _ => unreachable!(),
    }

    Ok(())
}
