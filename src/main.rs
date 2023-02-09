#![allow(clippy::manual_map)]

mod backoff;
mod client;
mod config;
mod err;
mod future;
mod heartbeat;
mod magic;
mod opt;
mod rw;
mod server;
mod stream;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), err::DebugFromDisplay<std::io::Error>> {
    let opt::Options { verbose, mode } = clap::Parser::parse();

    env_logger::Builder::new()
        .filter_level(match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        })
        .init();

    let local = tokio::task::LocalSet::new();

    match mode {
        opt::Mode::Server { gateway, public } => {
            local
                .run_until(server::run(&local, &gateway, &public))
                .await?;
        }
        opt::Mode::Client { gateway, private } => {
            local
                .run_until(client::run(&local, &gateway, &private))
                .await;
        }
    }

    Ok(())
}
