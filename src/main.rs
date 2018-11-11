#![cfg_attr(not(feature = "cargo-clippy"), allow(unknown_lints))]
#![allow(unit_arg)]

extern crate env_logger;
extern crate futures;
extern crate log;
extern crate structopt;
extern crate tokio;

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

#[global_allocator]
static ALLOC: std::alloc::System = std::alloc::System;

fn main() -> Result<(), err::DebugFromDisplay<std::io::Error>> {
    use structopt::StructOpt;

    let opt::Options { verbose, mode } = opt::Options::from_args();

    env_logger::Builder::new()
        .filter_level(match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }).init();

    match mode {
        opt::Mode::Server { public, gateway } => server::run(
            &([0, 0, 0, 0], public).into(),
            &([0, 0, 0, 0], gateway).into(),
        )?,
        opt::Mode::Client {
            gateway,
            private,
            retry,
        } => client::run(&gateway.0, &private.0, retry)?,
    }

    Ok(())
}
