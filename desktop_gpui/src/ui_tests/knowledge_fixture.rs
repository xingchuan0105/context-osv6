use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct KnowledgeFixture {
    workspaces: Vec<Value>,
    documents: Vec<Value>,
    notes: Vec<Value>,
    pub fail_notes: Arc<AtomicBool>,
    pub fail_upload: Arc<AtomicBool>,
}

fn workspace(id: &str, name: &str) -> Value {
    json!({"id":id,"owner_user_id":"ui-user","owner_id":"ui-user","name":name,"title":name,"description":"","created_at":"2026-09-10","updated_at":"2026-09-10","document_count":1})
}
fn document(id: &str, wid: &str, name: &str, status: &str) -> Value {
    json!({"id":id,"workspace_id":wid,"owner_user_id":"ui-user","owner_id":"ui-user","file_name":name,"mime_type":"text/plain","file_size":8,"status":status,"chunk_count":1,"created_at":"2026-09-10","updated_at":"2026-09-10"})
}
impl KnowledgeFixture {
    pub fn new() -> Self {
        Self {
            workspaces: vec![
                workspace("workspace-ui", "资料验收"),
                workspace("workspace-two", "另一工作区"),
            ],
            documents: vec![
                document("ui-document", "workspace-ui", "年度资料.txt", "completed"),
                document("failed-document", "workspace-ui", "失败资料.txt", "failed"),
                document(
                    "other-document",
                    "workspace-two",
                    "另一资料.txt",
                    "completed",
                ),
            ],
            notes: vec![],
            fail_notes: Arc::new(AtomicBool::new(false)),
            fail_upload: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn respond(&mut self, method: &str, path: &str, body: &Value) -> Option<(u16, Value)> {
        let parts: Vec<_> = path.split('/').collect();
        if method == "GET" && path.starts_with("/api/v1/chat/sessions?workspace_id=") {
            let wid = path.split('=').nth(1).unwrap();
            return Some((
                200,
                json!({"sessions": if wid == "workspace-ui" { vec![json!({"id":"workspace-history","title":"工作区历史","workspace_id":wid,"owner_user_id":"ui-user","scope_kind":"workspace","model_role":"agent","agent_type":"rag","created_at":"2026-09-10","updated_at":"2026-09-10"})] } else { vec![] }}),
            ));
        }
        if path == "/api/v1/chat/sessions/workspace-history/messages" {
            return Some((
                200,
                json!({"messages":[{"id":20,"session_id":"workspace-history","role":"assistant","content":"工作区历史答案","citations":[{"citation_id":1,"doc_id":"ui-document","doc_name":"年度资料.txt","score":1.0}],"created_at":"2026-09-10"}]}),
            ));
        }
        if path == "/api/v1/workspaces" {
            if method == "POST" {
                let created = workspace("created-workspace", body["name"].as_str().unwrap());
                self.workspaces.push(created.clone());
                return Some((200, json!({"workspace":created})));
            }
            return Some((200, json!({"workspaces":self.workspaces})));
        }
        if method == "GET" && path.starts_with("/api/v1/documents?workspace_id=") {
            let wid = path.split('=').nth(1).unwrap();
            for document in self
                .documents
                .iter_mut()
                .filter(|d| d["workspace_id"] == wid)
            {
                if document["status"] == "processing" {
                    document["status"] = "completed".into();
                } else if document["status"] == "queued" {
                    document["status"] = "processing".into();
                }
            }
            return Some((
                200,
                json!({"documents": self.documents.iter().filter(|d| d["workspace_id"] == wid).collect::<Vec<_>>()}),
            ));
        }
        if parts.get(3) == Some(&"workspaces") && parts.len() >= 6 {
            let wid = parts[4];
            if parts[5] == "documents" && method == "POST" {
                assert!(self.workspaces.iter().any(|w| w["id"] == wid));
                self.documents.push(document(
                    "uploaded-document",
                    wid,
                    body["filename"].as_str().unwrap(),
                    "pending",
                ));
                return Some((
                    200,
                    json!({"document_id":"uploaded-document","upload_url":"/uploads/uploaded-document?signature=synthetic","status":"pending"}),
                ));
            }
            if parts[5] == "notes" {
                let id = parts.get(6).copied().unwrap_or("created-note");
                if method == "GET" {
                    return Some((
                        200,
                        json!({"notes":self.notes.iter().filter(|n| n["workspace_id"] == wid).collect::<Vec<_>>()}),
                    ));
                }
                if self.fail_notes.load(Ordering::SeqCst) {
                    return Some((500, json!({"error":"synthetic note save failed"})));
                }
                self.notes.retain(|n| n["id"] != id);
                if method == "DELETE" {
                    return Some((200, json!({"ok":true})));
                }
                let note = json!({"id":id,"workspace_id":wid,"title":body["title"],"content":body["content"],"preview":body["content"],"created_at":"2026-09-10","updated_at":"2026-09-10"});
                self.notes.push(note.clone());
                return Some((200, json!({"note":note})));
            }
        }
        if path.starts_with("/uploads/") {
            assert_eq!(method, "PUT");
            assert!(body["size"].as_u64().unwrap() > 0);
            return Some(if self.fail_upload.load(Ordering::SeqCst) {
                (503, json!({"error":"synthetic upload failed"}))
            } else {
                (200, json!({"ok":true}))
            });
        }
        if parts.get(3) == Some(&"documents") && parts.len() >= 5 {
            let id = parts[4];
            if parts.get(5) == Some(&"content") {
                return Some((
                    200,
                    json!({"content":"原文核对内容：年度资料。","summary":null}),
                ));
            }
            if method == "DELETE" {
                self.documents.retain(|d| d["id"] != id);
            } else if let Some(document) = self.documents.iter_mut().find(|d| d["id"] == id) {
                document["status"] = "queued".into();
            }
            return Some((200, json!({"ok":true,"status":"queued"})));
        }
        None
    }
}
