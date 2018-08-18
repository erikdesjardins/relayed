extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;
#[macro_use]
extern crate structopt;
extern crate tokio;

mod client;
mod conjoin;
mod opt;
mod server;
mod util;

fn main() -> Result<(), util::ShowCauses> {
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
        })
        .init();

    Ok(match mode {
        opt::Mode::Server => server::run(&from, &to),
        opt::Mode::Client => client::run(&from, &to, retry),
    }?)
}
