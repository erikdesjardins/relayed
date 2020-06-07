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

    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()?;
    let local = tokio::task::LocalSet::new();

    match mode {
        opt::Mode::Server { gateway, public } => {
            runtime.block_on(local.run_until(server::run(&local, &gateway, &public)))?;
        }
        opt::Mode::Client {
            gateway,
            private,
            retry,
        } => {
            runtime.block_on(local.run_until(client::run(&local, &gateway, &private, retry)))?;
        }
    }

    Ok(())
}
