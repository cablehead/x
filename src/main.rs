use anyhow::Result;
use clap::Command;

mod exec;
mod log;
mod stream;

fn main() -> Result<()> {
    let matches = Command::new("x")
        .version("0.0.4")
        .about("Swiss army knife for the command line")
        .subcommand_required(true)
        .disable_help_subcommand(true)
        .subcommand(exec::configure_app(Command::new("exec")))
        .subcommand(log::configure_app(Command::new("log")))
        .subcommand(stream::configure_app(Command::new("stream")))
        .get_matches();

    match matches.subcommand() {
        Some(("exec", matches)) => exec::run(matches)?,
        Some(("log", matches)) => log::run(matches)?,
        Some(("stream", matches)) => stream::run(matches)?,
        _ => unreachable!(),
    }

    Ok(())
}
