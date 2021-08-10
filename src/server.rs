use axum::{prelude::*, response::Json, routing::BoxRoute};
use cut_optimizer_2d::{CutPiece, Optimizer, Solution, StockPiece};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::oneshot;
use tower::{BoxError, ServiceBuilder};
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use tracing::error;

use crate::Opt;

#[cfg(test)]
mod tests;

/// Run optimizer server
pub(crate) async fn serve(socket_addr: SocketAddr, opt: &Opt) {
    // run it with hyper on localhost:3000
    hyper::Server::bind(&socket_addr)
        .serve(app(opt).into_make_service())
        .await
        .unwrap();
}

fn app(opt: &Opt) -> BoxRoute<Body> {
    let middleware_stack = ServiceBuilder::new()
        // Return an error after 30 seconds
        .timeout(Duration::from_secs(opt.timeout))
        // Shed load if we're receiving too many requests
        .load_shed()
        // Process at most 100 requests concurrently
        .concurrency_limit(opt.max_requests)
        .layer(TraceLayer::new_for_http())
        // Compress response bodies
        .layer(CompressionLayer::new())
        .into_inner();

    route("/optimize", post(optimize))
        .layer(middleware_stack)
        .handle_error(|error: BoxError| {
            let result = if error.is::<tower::timeout::error::Elapsed>() {
                Ok(StatusCode::REQUEST_TIMEOUT)
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!(&format!("Unhandled internal error: {:?}", &error))),
                ))
            };

            Ok::<_, Infallible>(result)
        })
        .boxed()
}

type OptimizeError = (StatusCode, Json<Value>);

/// Run optimizer in a thread pool
async fn optimize(input: extract::Json<OptimizerInput>) -> Result<Json<Solution>, OptimizeError> {
    let input = input.0;

    let (tx, rx) = oneshot::channel();

    rayon::spawn(move || {
        let method = input.method;
        let optimizer: Optimizer = input.into();
        let result = match method {
            OptimizeMethod::Guillotine => optimizer.optimize_guillotine(|_| {}),
            OptimizeMethod::Nested => optimizer.optimize_nested(|_| {}),
        };
        if tx.send(result).is_err() {
            error!("Error: receiver side of channel closed before the result could be sent.");
        }
    });

    let result = rx.await.map_err(|e| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't receive result from channel",
            e.to_string(),
        )
    })?;

    let solution = result.map_err(|e| match e {
        cut_optimizer_2d::Error::NoFitForCutPiece(cut_piece) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "The following cut piece doesn't fit in any stock pieces",
            cut_piece,
        ),
    })?;

    Ok(Json(solution))
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

impl From<OptimizerInput> for Optimizer {
    fn from(input: OptimizerInput) -> Self {
        let mut optimizer = Optimizer::new();
        optimizer
            .set_random_seed(input.random_seed.unwrap_or(1))
            .set_cut_width(input.cut_width)
            .add_stock_pieces(input.stock_pieces)
            .add_cut_pieces(input.cut_pieces)
            .allow_mixed_stock_sizes(input.allow_mixed_stock_sizes.unwrap_or(true));
        optimizer
    }
}

fn error<T: Serialize>(status_code: StatusCode, message: &str, data: T) -> OptimizeError {
    (
        status_code,
        Json(json!({ "message": message, "data": data })),
    )
}
