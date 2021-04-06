use cut_optimizer_2d::{CutPiece, Optimizer, StockPiece};
use env_logger::Env;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr};
use structopt::StructOpt;
use tokio::sync::oneshot;
use warp::{
    hyper::StatusCode,
    reply::{Json, WithStatus},
    Filter,
};

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
struct ErrorMessage<T: Serialize> {
    message: String,
    data: T,
}

#[derive(Serialize, Debug)]
struct ApiError<T: Serialize> {
    error: ErrorMessage<T>,
}

impl<T: Serialize> ApiError<T> {
    fn new(message: String, data: T) -> Self {
        ApiError {
            error: ErrorMessage { message, data },
        }
    }
}

fn error_reply<T: Serialize>(
    message: String,
    data: T,
    status_code: StatusCode,
) -> WithStatus<Json> {
    warp::reply::with_status(
        warp::reply::json(&ApiError::new(message, data)),
        status_code,
    )
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

    init_logger(&opt);

    let api = optimize_filter(opt.max_content_length).with(warp::filters::log::custom(|info| {
        info!("{} {} {}", info.method(), info.path(), info.status());
    }));

    warp::serve(api).run(socket_address(&opt)).await;
}

fn socket_address(opt: &Opt) -> SocketAddr {
    format!("{}:{}", opt.ip, opt.port).parse().unwrap()
}

fn init_logger(opt: &Opt) {
    if !opt.quiet {
        env_logger::Builder::from_env(Env::default().default_filter_or(match opt.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }))
        .init();
    }
}

/// POST /optimize with JSON body
fn optimize_filter(
    max_content_length: u64,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("optimize")
        .and(warp::filters::method::post())
        .and(warp::body::content_length_limit(max_content_length))
        .and(warp::body::json())
        .and_then(optimize)
}

/// Run optimizer in a thread pool
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
            Ok(solution) => Ok(warp::reply::with_status(
                warp::reply::json(&solution),
                StatusCode::OK,
            )),
            Err(cut_optimizer_2d::Error::NoFitForCutPiece(cut_piece)) => Ok(error_reply(
                "The following cut piece doesn't fit any stock pieces".to_string(),
                cut_piece.clone(),
                StatusCode::UNPROCESSABLE_ENTITY,
            )),
        },
        Err(e) => Ok(error_reply(
            "Couldn't receive result from channel".to_string(),
            e.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::optimize_filter;
    use warp::{hyper::StatusCode, test::request};

    static TEST_INPUT: &str = r#"
        {
            "method": "guillotine",
            "randomSeed": 1,
            "cutWidth": 2,
            "stockPieces": [
                {
                    "width": 48,
                    "length": 96,
                    "patternDirection": "none"
                },
                {
                    "width": 48,
                    "length": 120,
                    "patternDirection": "none"
                }
            ],
            "cutPieces": [
                {
                    "externalId": 1,
                    "width": 10,
                    "length": 30,
                    "patternDirection": "none",
                    "canRotate": true
                },
                {
                    "externalId": 2,
                    "width": 45,
                    "length": 100,
                    "patternDirection": "none",
                    "canRotate": true
                }
            ]
        }
    "#;

    #[tokio::test]
    async fn optimize_should_return_ok() {
        let api = optimize_filter(10240);
        let resp = request()
            .method("POST")
            .path("/optimize")
            .body(&TEST_INPUT)
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn content_length_too_long_should_return_payload_too_large() {
        let api = optimize_filter(100);
        let resp = request()
            .method("POST")
            .path("/optimize")
            .body(&TEST_INPUT)
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    async fn optimize_with_wrong_http_method(http_method: &str) {
        let api = optimize_filter(10240);
        let resp = request()
            .method(http_method)
            .path("/optimize")
            .body(&TEST_INPUT)
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn optimize_with_delete_should_fail() {
        optimize_with_wrong_http_method("DELETE").await
    }

    #[tokio::test]
    async fn optimize_with_get_should_fail() {
        optimize_with_wrong_http_method("GET").await
    }

    #[tokio::test]
    async fn optimize_with_patch_should_fail() {
        optimize_with_wrong_http_method("PATCH").await
    }

    #[tokio::test]
    async fn optimize_with_put_should_fail() {
        optimize_with_wrong_http_method("PUT").await
    }

    #[tokio::test]
    async fn invalid_input_should_return_bad_request() {
        let api = optimize_filter(1024);
        let invalid_input = "{}";
        let resp = request()
            .method("POST")
            .path("/optimize")
            .body(&invalid_input)
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn non_fitting_price_should_return_unprocessable_entity() {
        let api = optimize_filter(1024);
        let non_fitting_input = r#"
            {
                "method": "guillotine",
                "randomSeed": 1,
                "cutWidth": 2,
                "stockPieces": [
                    {
                        "width": 48,
                        "length": 96,
                        "patternDirection": "none"
                    }
                ],
                "cutPieces": [
                    {
                        "externalId": 1,
                        "width": 10,
                        "length": 300,
                        "patternDirection": "none",
                        "canRotate": true
                    }
                ]
            }
        "#;
        let resp = request()
            .method("POST")
            .path("/optimize")
            .body(&non_fitting_input)
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
