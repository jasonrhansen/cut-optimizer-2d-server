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
    let resp = request()
        .method("POST")
        .path("/optimize")
        .body(&non_fitting_input)
        .reply(&api)
        .await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
