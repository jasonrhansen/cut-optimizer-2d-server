use cut_optimizer_2d::{CutPiece, Optimizer, StockPiece};
use env_logger::Env;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr};
use structopt::StructOpt;
use tokio::sync::oneshot;
use warp::Filter;

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum OptimizeMethod {
    Guillotine,
    Nested,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OptimizerInput {
    method: OptimizeMethod,
    random_seed: Option<u64>,
    cut_width: usize,
    stock_pieces: Vec<StockPiece>,
    cut_pieces: Vec<CutPiece>,
}

impl Into<Optimizer> for OptimizerInput {
    fn into(self) -> Optimizer {
        let mut optimizer = Optimizer::new();
        optimizer
            .set_random_seed(self.random_seed.unwrap_or(1))
            .set_cut_width(self.cut_width)
            .add_stock_pieces(self.stock_pieces)
            .add_cut_pieces(self.cut_pieces);
        optimizer
    }
}

#[derive(Serialize, Debug)]
struct ErrorMessage {
    error: String,
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cut-optimizer-2d-server",
    about = "A cut optimizer server for optimizing rectangular cut pieces from sheet goods.",
    author = "Jason Hansen <jasonrodneyhansen@gmail.com>"
)]
struct Opt {
    /// IP address to listen on
    #[structopt(short = "i", long = "ip", default_value = "127.0.0.1")]
    ip: String,

    /// Port to listen on
    #[structopt(short = "p", long = "port", default_value = "3030")]
    port: u16,

    /// Maximum length of request body
    #[structopt(long = "max-content-length", default_value = "32896")]
    max_content_length: u64,

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

    // Initialize logger
    if !opt.quiet {
        env_logger::Builder::from_env(Env::default().default_filter_or(match opt.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }))
        .init();
    }

    let addr: SocketAddr = format!("{}:{}", opt.ip, opt.port).parse().unwrap();

    let optimize = warp::path!("optimize")
        .and(warp::filters::method::post())
        .and(warp::body::content_length_limit(opt.max_content_length))
        .and(warp::body::json())
        .and_then(optimize)
        .with(warp::filters::log::custom(|info| {
            info!("{} {} {}", info.method(), info.path(), info.status());
        }));

    warp::serve(optimize).run(addr).await;
}

async fn optimize(input: OptimizerInput) -> Result<impl warp::Reply, Infallible> {
    let (tx, rx) = oneshot::channel();

    rayon::spawn(move || {
        let method = input.method;
        let optimizer: Optimizer = input.into();
        let result = match method {
            OptimizeMethod::Guillotine => optimizer.optimize_guillotine(|_| {}),
            OptimizeMethod::Nested => optimizer.optimize_nested(|_| {}),
        };
        if let Err(_) = tx.send(result) {
            error!("Error: receiver side of channel closed before the result could be sent.");
        }
    });

    match rx.await {
        Ok(result) => match result {
            Ok(solution) => Ok(warp::reply::json(&solution)),
            Err(cut_optimizer_2d::Error::NoFitForCutPiece(cut_piece)) => {
                Ok(warp::reply::json(&ErrorMessage {
                    error: format!(
                        "The following cut piece doesn't fit any stock pieces: {:?}",
                        cut_piece
                    ),
                }))
            }
        },
        Err(e) => Ok(warp::reply::json(&ErrorMessage {
            error: e.to_string(),
        })),
    }
}
