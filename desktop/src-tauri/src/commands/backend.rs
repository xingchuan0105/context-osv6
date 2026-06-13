use common::Document;

pub fn backend_status_payload(initialized: bool) -> serde_json::Value {
    serde_json::json!({
        "initialized": initialized,
        "type": "local",
        "storage": {
            "type": "filesystem",
            "initialized": initialized
        },
        "cache": {
            "type": "memory",
            "initialized": initialized
        }
    })
}

pub fn local_document_json(doc: &Document) -> serde_json::Value {
    serde_json::json!({
        "id": doc.id,
        "name": doc.file_name,
        "status": doc.status,
        "created_at": doc.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::documents::DocumentStatus;

    fn sample_document() -> Document {
        Document {
            id: "doc-1".to_string(),
            org_id: "org-1".to_string(),
            notebook_id: "nb-1".to_string(),
            owner_id: "user-1".to_string(),
            file_name: "report.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            file_size: 1024,
            status: DocumentStatus::Completed,
            chunk_count: 3,
            created_at: "2026-06-13T00:00:00Z".to_string(),
            updated_at: "2026-06-13T01:00:00Z".to_string(),
        }
    }

    #[test]
    fn backend_status_payload_reflects_initialization_state() {
        let initialized = backend_status_payload(true);
        assert_eq!(initialized["initialized"], true);
        assert_eq!(initialized["type"], "local");
        assert_eq!(initialized["storage"]["initialized"], true);
        assert_eq!(initialized["cache"]["type"], "memory");

        let uninitialized = backend_status_payload(false);
        assert_eq!(uninitialized["initialized"], false);
        assert_eq!(uninitialized["storage"]["initialized"], false);
        assert_eq!(uninitialized["cache"]["initialized"], false);
    }

    #[test]
    fn local_document_json_maps_ipc_list_fields() {
        let json = local_document_json(&sample_document());

        assert_eq!(json["id"], "doc-1");
        assert_eq!(json["name"], "report.pdf");
        assert_eq!(json["status"], "completed");
        assert_eq!(json["created_at"], "2026-06-13T00:00:00Z");
        assert!(json.get("org_id").is_none());
    }
}
