use std::net::SocketAddr;
use structopt::StructOpt;
use tracing::error;
use tracing_subscriber::EnvFilter;

mod server;

#[derive(Default, Debug, StructOpt)]
#[structopt(
    name = "cut-optimizer-2d-server",
    about = "A cut optimizer server for optimizing rectangular cut pieces from sheet goods.",
    author = "Jason Hansen <jasonrodneyhansen@gmail.com>"
)]
pub(crate) struct Opt {
    /// IP address to listen on
    #[structopt(
        short = "i",
        long = "ip",
        default_value = "0.0.0.0",
        env = "CUT_OPTIMIZER_2D_IP"
    )]
    ip: String,

    /// Port to listen on
    #[structopt(
        short = "p",
        long = "port",
        default_value = "3030",
        env = "CUT_OPTIMIZER_2D_PORT"
    )]
    port: u16,

    /// Timeout in seconds
    #[structopt(long = "timeout", default_value = "60", env = "CUT_OPTIMIZER_TIMEOUT")]
    timeout: u64,

    /// Maximum number of concurrent requests
    #[structopt(
        long = "max-requests",
        default_value = "100",
        env = "CUT_OPTIMIZER_MAX_REQUESTS"
    )]
    max_requests: usize,

    /// Silence all log output
    #[structopt(short = "q", long = "quiet")]
    quiet: bool,

    /// Verbose logging mode (-v, -vv, -vvv)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();

    init_tracing(&opt);

    let addr = format!("{}:{}", opt.ip, opt.port);
    if let Ok(socket_addr) = addr.parse::<SocketAddr>() {
        server::serve(socket_addr, &opt).await;
    } else {
        error!("Error parsing socket address: {}", addr);
    }
}

fn init_tracing(opt: &Opt) {
    if !opt.quiet {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var(
                "RUST_LOG",
                match opt.verbose {
                    0 => "warn",
                    1 => "info",
                    2 => "debug",
                    _ => "trace",
                },
            )
        }
        tracing_subscriber::fmt::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }
}
