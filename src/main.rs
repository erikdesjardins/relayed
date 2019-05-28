#![allow(clippy::unit_arg)]

mod backoff;
mod client;
mod config;
mod err;
mod future;
mod heartbeat;
mod magic;
mod never;
mod opt;
mod rw;
mod server;
mod stream;
mod tcp;

fn main() -> Result<(), err::DebugFromDisplay<std::io::Error>> {
    use structopt::StructOpt;

    let opt::Options { verbose, mode } = opt::Options::from_args();

    env_logger::Builder::new()
        .filter_level(match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        })
        .init();

    match mode {
        opt::Mode::Server { gateway, public } => server::run(&gateway, &public)?,
        opt::Mode::Client {
            gateway,
            private,
            retry,
        } => client::run(&gateway, &private, retry)?,
    }

    Ok(())
}
