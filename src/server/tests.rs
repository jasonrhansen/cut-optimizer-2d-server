use super::*;
use http::{Request, StatusCode};
use structopt::StructOpt;
use tower::ServiceExt;

static TEST_INPUT: &str = r#"
    {
        "method": "guillotine",
        "randomSeed": 1,
        "cutWidth": 2,
        "stockPieces": [
            {
                "width": 48,
                "length": 96,
                "patternDirection": "none",
                "price": 0
            },
            {
                "width": 48,
                "length": 120,
                "patternDirection": "none",
                "price": 0
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

fn test_app() -> Router<Body> {
    app(&Opt::from_iter(&[
        "cut-optimizer-2d-server",
        "--timeout",
        "60",
        "--max-requests",
        "100",
    ]))
}

#[tokio::test]
async fn optimize_should_return_ok() {
    let resp = test_app()
        .oneshot(
            Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri("/optimize")
                .body(TEST_INPUT.into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

async fn optimize_with_wrong_http_method(http_method: &str) {
    let resp = test_app()
        .oneshot(
            Request::builder()
                .header("Content-Type", "application/json")
                .method(http_method)
                .uri("/optimize")
                .body(TEST_INPUT.into())
                .unwrap(),
        )
        .await
        .unwrap();

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
    let invalid_input = "{}";

    let resp = test_app()
        .oneshot(
            Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri("/optimize")
                .body(invalid_input.into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn non_fitting_price_should_return_unprocessable_entity() {
    let non_fitting_input = r#"
        {
            "method": "guillotine",
            "randomSeed": 1,
            "cutWidth": 2,
            "stockPieces": [
                {
                    "width": 48,
                    "length": 96,
                    "patternDirection": "none",
                    "price": 0
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

    let resp = test_app()
        .oneshot(
            Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri("/optimize")
                .body(non_fitting_input.into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
