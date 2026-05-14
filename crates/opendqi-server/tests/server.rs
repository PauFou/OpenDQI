//! Integration tests for the OpenDQI desktop web UI router.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use opendqi_server::{router, AppState};
use tower::ServiceExt;

fn build_router() -> axum::Router {
    let state = Arc::new(AppState::new().expect("create app state"));
    router(state)
}

#[tokio::test]
async fn get_root_renders_upload_form() {
    let app = build_router();
    let req = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("OpenDQI desktop"));
    assert!(html.contains("enctype=\"multipart/form-data\""));
    assert!(html.contains("name=\"regime\""));
    assert!(html.contains("name=\"file\""));
}

#[tokio::test]
async fn get_unknown_scan_returns_404() {
    let app = build_router();
    let req = Request::builder()
        .method(Method::GET)
        .uri("/scans/does-not-exist")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_scan_with_invalid_id_is_bad_request() {
    let app = build_router();
    // path traversal attempt
    let req = Request::builder()
        .method(Method::GET)
        .uri("/scans/..%2F..")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // axum percent-decodes, so this hits safe_join's reject path.
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND,
        "unexpected status: {}",
        resp.status()
    );
}

#[tokio::test]
async fn post_scan_with_emir_xml_succeeds_and_results_page_renders() {
    let app = build_router();
    let xml = std::fs::read("../../examples/emir/sample.xml").expect("sample.xml fixture");
    let boundary = "------OpenDqiTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"regime\"\r\n\r\nemir\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"sample.xml\"\r\nContent-Type: application/xml\r\n\r\n",
    );
    body.extend_from_slice(&xml);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/scan")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|h| h.to_str().ok())
        .expect("redirect location header")
        .to_string();
    assert!(location.starts_with("/scans/"), "got location: {location}");

    // Fetch the results page.
    let req = Request::builder()
        .method(Method::GET)
        .uri(&location)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("Scan results"));
    assert!(html.contains("EMIR"));
    assert!(html.contains("report.html"));

    // Fetch the issues.csv artifact.
    let issues_uri = format!("{location}/issues.csv");
    let req = Request::builder()
        .method(Method::GET)
        .uri(&issues_uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(ctype.contains("csv"));
}

#[tokio::test]
async fn post_scan_with_invalid_regime_is_400() {
    let app = build_router();
    let boundary = "------OpenDqiTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"regime\"\r\n\r\nxyz\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"a.xml\"\r\n\r\n<x/>\r\n",
    );
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/scan")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn post_scan_with_operation_tr_state_emir_succeeds() {
    let app = build_router();
    let xml =
        std::fs::read("../../examples/emir/tr_state/auth107-sample.xml").expect("auth107 fixture");
    let boundary = "------OpenDqiTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"operation\"\r\n\r\ntr-state-scan\r\n",
    );
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"regime\"\r\n\r\nemir\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"auth107.xml\"\r\nContent-Type: application/xml\r\n\r\n",
    );
    body.extend_from_slice(&xml);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/scan")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn post_scan_with_recon_stats_emir_succeeds() {
    let app = build_router();
    let xml = std::fs::read("../../examples/emir/recon_stats/auth091-sample.xml")
        .expect("auth091 fixture");
    let boundary = "------OpenDqiTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"operation\"\r\n\r\nrecon-stats\r\n",
    );
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"regime\"\r\n\r\nemir\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"auth091.xml\"\r\nContent-Type: application/xml\r\n\r\n",
    );
    body.extend_from_slice(&xml);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/scan")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}
