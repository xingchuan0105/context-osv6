//! Alipay F2F smoke test: calls alipay.trade.precreate for ¥0.01 against the
//! gateway configured in the given env file (default `.env` in cwd).
//!
//! Usage:
//!   cargo run -p avrag-billing --example alipay_precreate_smoke -- path/to/.env
//!
//! A returned qr_code proves: app_id valid, private-key signing accepted, and
//! 当面付 product permission active. The order is unpaid and expires on its own.

use avrag_billing::{AlipayClient, BillingConfig};

fn load_alipay_config(path: &str) -> BillingConfig {
    let mut config = BillingConfig::default();
    let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(2);
    });
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "ALIPAY_APP_ID" => config.alipay_app_id = value.to_string(),
            "ALIPAY_PRIVATE_KEY" => config.alipay_private_key = value.to_string(),
            "ALIPAY_PUBLIC_KEY" => config.alipay_public_key = value.to_string(),
            "ALIPAY_GATEWAY_URL" => config.alipay_gateway_url = value.to_string(),
            "ALIPAY_NOTIFY_URL" => config.alipay_notify_url = Some(value.to_string()),
            _ => {}
        }
    }
    config
}

#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".env".to_string());
    let config = load_alipay_config(&path);
    assert!(config.alipay_enabled(), "ALIPAY_APP_ID missing in {path}");

    let out_trade_no = format!("smoke{}", chrono::Utc::now().timestamp());
    let notify_url = config.alipay_notify_url.clone().unwrap_or_default();
    println!(
        "precreate ¥0.01 via {} (app_id={}…) out_trade_no={out_trade_no}",
        config.alipay_gateway_url,
        &config.alipay_app_id[..config.alipay_app_id.len().min(6)]
    );

    let client = AlipayClient::new(config);
    match client
        .create_precreate_order("0.01", "ContextOS smoke test", &out_trade_no, &notify_url)
        .await
    {
        Ok(qr_code) => {
            println!("OK — qr_code: {}…", &qr_code[..qr_code.len().min(48)]);
        }
        Err(error) => {
            eprintln!("FAILED: {error}");
            std::process::exit(1);
        }
    }
}
