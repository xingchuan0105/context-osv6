use contracts::documents::SessionFileRow;
use web_sdk::{
    BrowserRestClient, TransportError, TrayFileStatus, complete_upload_url, create_session_json,
    create_upload_json, files_block_send, parse_session_files, parse_upload_response,
    ready_file_count, resolve_upload_url, session_file_url, session_files_url, tray_status,
    tray_status_attr,
};

fn row(status: &str) -> SessionFileRow {
    SessionFileRow {
        binding_id: "bind-1".into(),
        document_id: "doc-1".into(),
        file_name: "notes.txt".into(),
        mime_type: "text/plain".into(),
        file_size: 5,
        status: status.into(),
        parse_version: None,
        created_at: "2026-09-04T00:00:00Z".into(),
    }
}

#[test]
fn session_file_urls_encode_ids() {
    assert_eq!(
        session_files_url("http://x/", "sess/1"),
        "http://x/api/v1/chat/sessions/sess%2F1/files"
    );
    assert_eq!(
        session_file_url("", "s1", "b/2"),
        "/api/v1/chat/sessions/s1/files/b%2F2"
    );
    assert_eq!(
        complete_upload_url("http://x", "doc 1"),
        "http://x/api/v1/documents/doc%201/complete-upload"
    );
    assert_eq!(
        resolve_upload_url("http://127.0.0.1:18081", "/uploads/doc-1"),
        "http://127.0.0.1:18081/uploads/doc-1"
    );
    assert_eq!(
        resolve_upload_url(
            "http://127.0.0.1:18081",
            "http://127.0.0.1:8080/uploads/doc-1?expires=123&signature=abc"
        ),
        "http://127.0.0.1:18081/uploads/doc-1?expires=123&signature=abc"
    );
    assert_eq!(
        resolve_upload_url(
            "http://127.0.0.1:18081",
            "https://s3.amazonaws.com/my-bucket/doc-1"
        ),
        "https://s3.amazonaws.com/my-bucket/doc-1"
    );
}

#[test]
fn create_bodies_are_product_json() {
    assert_eq!(create_session_json(), b"{}");
    let body = create_upload_json("notes.txt", 12, "text/plain").expect("json");
    let value: serde_json::Value = serde_json::from_slice(&body).expect("parse");
    assert_eq!(value["filename"], "notes.txt");
    assert_eq!(value["file_size"], 12);
    assert_eq!(value["mime_type"], "text/plain");
}

#[test]
fn parse_list_and_presign_from_wire() {
    let list = parse_session_files(
        br#"{"files":[{"binding_id":"b1","document_id":"d1","file_name":"a.md","mime_type":"text/markdown","file_size":3,"status":"completed","created_at":"2026-09-04T00:00:00Z"}]}"#,
    )
    .expect("files");
    assert_eq!(list.files.len(), 1);
    assert_eq!(list.files[0].file_name, "a.md");

    let upload = parse_upload_response(
        br#"{"document_id":"d1","upload_url":"http://127.0.0.1:3201/upload/d1","status":"pending"}"#,
    )
    .expect("upload");
    assert_eq!(upload.document_id, "d1");
    assert!(upload.upload_url.contains("/upload/d1"));
}

#[test]
fn tray_status_and_send_gate() {
    assert_eq!(tray_status("completed"), TrayFileStatus::Ready);
    assert_eq!(tray_status("pending"), TrayFileStatus::Uploading);
    assert_eq!(tray_status("processing"), TrayFileStatus::Parsing);
    assert_eq!(tray_status("failed"), TrayFileStatus::Failed);
    assert_eq!(tray_status_attr(TrayFileStatus::Ready), "ready");
    assert!(!files_block_send(&[row("completed"), row("failed")]));
    assert!(files_block_send(&[row("processing")]));
    assert!(files_block_send(&[row("queued")]));
    assert_eq!(ready_file_count(&[row("completed"), row("processing")]), 1);
    assert_eq!(ready_file_count(&[row("failed")]), 0);
}

#[tokio::test]
async fn native_session_file_rest_is_unavailable() {
    let client = BrowserRestClient::new("http://127.0.0.1:18081", Some("token".to_string()));
    let err = client
        .list_session_files("sess-1")
        .await
        .expect_err("native unavailable");
    assert!(matches!(err, TransportError::Unavailable(_)));
}
