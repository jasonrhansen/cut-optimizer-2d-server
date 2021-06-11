use cut_optimizer_2d::{CutPiece, Optimizer, StockPiece};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr};
use tokio::sync::oneshot;
use warp::{
    hyper::StatusCode,
    reply::{Json, WithStatus},
    Filter,
};

#[cfg(test)]
mod tests;

/// Run optimizer server
pub(crate) fn serve(socket_addr: SocketAddr, max_content_length: u64) -> impl warp::Future {
    let api = optimize_filter(max_content_length)
        .or(root())
        .with(warp::filters::log::custom(|info| {
            info!("{} {} {}", info.method(), info.path(), info.status());
        }));

    warp::serve(api).run(socket_addr)
}

/// POST /optimize with JSON body
fn optimize_filter(
    max_content_length: u64,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("optimize")
        .and(warp::filters::method::post())
        .and(warp::body::content_length_limit(max_content_length))
        .and(warp::body::json())
        .and_then(optimize)
}

/// GET /
fn root() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path::end().and(warp::filters::method::get()).map(|| {
        "Cut Optimizer"
    })
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
    allow_mixed_stock_sizes: Option<bool>,
}

impl Into<Optimizer> for OptimizerInput {
    fn into(self) -> Optimizer {
        let mut optimizer = Optimizer::new();
        optimizer
            .set_random_seed(self.random_seed.unwrap_or(1))
            .set_cut_width(self.cut_width)
            .add_stock_pieces(self.stock_pieces)
            .add_cut_pieces(self.cut_pieces)
            .allow_mixed_stock_sizes(self.allow_mixed_stock_sizes.unwrap_or(true));
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
