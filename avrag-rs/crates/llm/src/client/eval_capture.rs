//! Opt-in local E2E evidence of prepared model inputs. Provider credentials
//! and the rest of ModelProviderConfig never enter the capture payload.

use super::ChatMessage;

pub(super) fn record(model: &str, feature: &str, stage: &str, messages: &[ChatMessage]) {
    if std::env::var("E2E_ENABLED").as_deref() != Ok("true") {
        return;
    }
    let Some(directory) = std::env::var_os("AVRAG_EVAL_LLM_TRACE_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    if !directory.is_absolute() {
        eprintln!("E2E model-input capture directory is not absolute");
        return;
    }
    let result = (|| -> anyhow::Result<()> {
        std::fs::create_dir_all(&directory)?;
        let payload = serde_json::json!({
            "kind": "prepared_model_input",
            "recorded_at_ms": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),
            "model": model,
            "feature": feature,
            "stage": stage,
            "messages": messages,
        });
        std::fs::write(
            directory.join(format!("{}.json", uuid::Uuid::new_v4())),
            serde_json::to_vec(&payload)?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("E2E model-input capture failed: {error}");
    }
}
