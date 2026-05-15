//! HTTP handlers: index form, scan upload, results page, file download.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use minijinja::context;
use serde_json::json;

use crate::scan::{
    new_scan_dir, run_server_dispatch, sanitize_upload_filename, UiOperation, UiRegime,
};
use crate::state::AppState;

/// `GET /` — render the upload form.
pub async fn index(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let tmpl = state.templates.get_template("index.html")?;
    let rendered = tmpl.render(context! {
        title => "OpenDQI desktop",
        body_class => "page-index",
    })?;
    Ok(Html(rendered))
}

/// `POST /api/scan` — accept multipart upload + form fields, run the
/// scan, then redirect to `/scans/{id}`.
pub async fn scan(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    use std::collections::BTreeMap;

    let mut regime: Option<UiRegime> = None;
    let mut operation: UiOperation = UiOperation::Scan;
    // Every file part, keyed by its form field name (`file`,
    // `file_book`, `file_tsr`, `file_mapping`, `file_tar`,
    // `file_feedback`). Value = (sanitized filename, bytes).
    let mut files: BTreeMap<String, (String, Vec<u8>)> = BTreeMap::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("multipart error: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "regime" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| AppError::bad_request(format!("reading regime: {e}")))?;
                regime = UiRegime::parse(&v);
            }
            "operation" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| AppError::bad_request(format!("reading operation: {e}")))?;
                operation = UiOperation::parse(&v)
                    .ok_or_else(|| AppError::bad_request(format!("invalid operation '{v}'")))?;
            }
            n if n == "file" || n.starts_with("file_") => {
                let fname = field
                    .file_name()
                    .map(sanitize_upload_filename)
                    .unwrap_or_else(|| "upload.bin".to_string());
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad_request(format!("reading {n}: {e}")))?;
                if !bytes.is_empty() {
                    files.insert(n.to_string(), (fname, bytes.to_vec()));
                }
            }
            _ => {
                // Drain unknown fields silently.
                let _ = field.bytes().await;
            }
        }
    }

    let regime = regime.ok_or_else(|| AppError::bad_request("missing or invalid regime"))?;
    if files.is_empty() {
        return Err(AppError::bad_request("no file uploaded"));
    }

    let (scan_id, scan_dir) =
        new_scan_dir(&state.temp_root).map_err(|e| AppError::server_error(e.to_string()))?;

    // Persist every uploaded part into the scan dir, building a
    // field-name -> saved-path map.
    let mut saved: BTreeMap<String, std::path::PathBuf> = BTreeMap::new();
    for (field_name, (fname, bytes)) in &files {
        let dest = scan_dir.join(fname);
        std::fs::write(&dest, bytes)
            .map_err(|e| AppError::server_error(format!("saving {field_name}: {e}")))?;
        saved.insert(field_name.clone(), dest);
    }

    // Run the scan synchronously. We hand off blocking work to a
    // tokio task-blocking thread so the runtime stays responsive.
    let dir_for_task = scan_dir.clone();
    let regime_copy = regime;
    let op_copy = operation;
    let artifacts = tokio::task::spawn_blocking(move || {
        run_server_dispatch(&saved, regime_copy, op_copy, &dir_for_task)
    })
    .await
    .map_err(|e| AppError::server_error(format!("scan task panicked: {e}")))?
    .map_err(|e| AppError::server_error(format!("scan failed: {e:#}")))?;

    // Persist a small JSON sidecar so the results page can re-render
    // without re-running the scan.
    let sidecar = scan_dir.join("scan_meta.json");
    let meta = json!({
        "regime": artifacts.regime.as_str(),
        "operation": operation.as_str(),
        "records": artifacts.records,
        "issues_total": artifacts.issues_total,
        "issues_critical": artifacts.issues_critical,
        "issues_high": artifacts.issues_high,
        "quality_score": artifacts.quality_score,
        "artifacts": artifacts.artifact_files,
        "upload_name": files.keys().cloned().collect::<Vec<_>>().join(", "),
    });
    std::fs::write(
        &sidecar,
        serde_json::to_vec_pretty(&meta).unwrap_or_default(),
    )
    .map_err(|e| AppError::server_error(format!("writing scan_meta.json: {e}")))?;

    Ok(Redirect::to(&format!("/scans/{scan_id}")).into_response())
}

/// `GET /scans/{id}` — render the scan results page.
pub async fn scan_results(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppError> {
    let scan_dir = state
        .safe_join(&id, None)
        .ok_or_else(|| AppError::bad_request("invalid scan id"))?;
    let sidecar = scan_dir.join("scan_meta.json");
    let raw = std::fs::read(&sidecar)
        .map_err(|_| AppError::not_found(format!("scan {id} not found or expired")))?;
    let meta: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| AppError::server_error(format!("reading scan_meta.json: {e}")))?;

    let tmpl = state.templates.get_template("results.html")?;
    let rendered = tmpl.render(context! {
        title => "OpenDQI scan results",
        body_class => "page-results",
        scan_id => id,
        meta => meta,
    })?;
    Ok(Html(rendered))
}

/// `GET /scans/{id}/{file}` — stream a report artifact.
pub async fn scan_download(
    State(state): State<Arc<AppState>>,
    Path((id, file)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let path = state
        .safe_join(&id, Some(&file))
        .ok_or_else(|| AppError::bad_request("invalid path"))?;
    let bytes =
        std::fs::read(&path).map_err(|_| AppError::not_found(format!("{file} not found")))?;
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file}\""),
        )
        .body(Body::from(bytes))
        .unwrap();
    Ok(resp)
}

// =================================================================
// Error type
// =================================================================

pub struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    fn server_error(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl From<minijinja::Error> for AppError {
    fn from(e: minijinja::Error) -> Self {
        Self::server_error(format!("template error: {e}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::warn!(status = %self.status, message = %self.message, "request failed");
        (self.status, format!("{}: {}\n", self.status, self.message)).into_response()
    }
}
