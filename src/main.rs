extern crate env_logger;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;
#[macro_use]
extern crate structopt;
extern crate tokio;

mod backoff;
mod client;
mod err;
mod opt;
mod server;
mod stream;
mod tcp;

fn main() -> Result<(), err::DebugFromDisplay<std::io::Error>> {
    use structopt::StructOpt;

    let opt::Options {
        verbose,
        retry,
        mode,
        from,
        to,
    } = opt::Options::from_args();

    env_logger::Builder::new()
        .filter_level(match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }).init();

    match mode {
        opt::Mode::Server => server::run(&from, &to)?,
        opt::Mode::Client => client::run(from, to, retry)?,
    }

    Ok(())
}
