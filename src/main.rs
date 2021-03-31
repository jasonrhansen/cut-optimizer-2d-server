use cut_optimizer_2d::{CutPiece, Optimizer, StockPiece};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::sync::oneshot;
use warp::Filter;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OptimizerInput {
    random_seed: u64,
    cut_width: usize,
    stock_pieces: Vec<StockPiece>,
    cut_pieces: Vec<CutPiece>,
}

#[tokio::main]
async fn main() {
    let optimize = warp::path!("optimize")
        .and(warp::body::content_length_limit(1024 * 32))
        .and(warp::body::json())
        .and_then(optimize);

    warp::serve(optimize).run(([127, 0, 0, 1], 3030)).await;
}

#[derive(Serialize, Debug)]
struct ErrorMessage {
    error: String,
}

async fn optimize(input: OptimizerInput) -> Result<impl warp::Reply, Infallible> {
    let (tx, rx) = oneshot::channel();

    tokio::task::spawn_blocking(move || {
        let result = Optimizer::new()
            .set_random_seed(input.random_seed)
            .set_cut_width(input.cut_width)
            .add_stock_pieces(input.stock_pieces)
            .add_cut_pieces(input.cut_pieces)
            .optimize_guillotine(|_| {});
        let _ = tx.send(result);
    })
    .await
    .unwrap();

    match rx.await {
        Ok(result) => match result {
            Ok(solution) => Ok(warp::reply::json(&solution)),
            Err(cut_optimizer_2d::Error::NoFitForCutPiece(cut_piece)) => {
                Ok(warp::reply::json(&ErrorMessage {
                    error: format!(
                        "Cut piece with external_id = {} doesn't fit any stock pieces",
                        cut_piece.external_id
                    ),
                }))
            }
        },
        Err(e) => Ok(warp::reply::json(&ErrorMessage {
            error: e.to_string(),
        })),
    }
}
