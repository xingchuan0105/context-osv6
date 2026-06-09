use crate::types::BillingConfig;
use anyhow::{Result, anyhow, bail};
use base64::Engine;
use rsa::pkcs1v15::{SigningKey, VerifyingKey};
use rsa::signature::{SignatureEncoding, Signer, Verifier};
use rsa::{
    RsaPrivateKey, RsaPublicKey,
    pkcs1::DecodeRsaPrivateKey,
    pkcs8::{DecodePrivateKey, DecodePublicKey},
};
use sha2::Sha256;

fn normalize_key_input(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=')
        .collect();
    cleaned
}

fn wrap_base64_64(b64: &str) -> String {
    b64.as_bytes()
        .chunks(64)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_private_key(raw: &str) -> Result<RsaPrivateKey> {
    let trimmed = raw.trim();
    if trimmed.contains("BEGIN RSA PRIVATE KEY") {
        return RsaPrivateKey::from_pkcs1_pem(trimmed)
            .map_err(|e| anyhow!("invalid alipay PKCS#1 private key: {}", e));
    }
    if trimmed.contains("BEGIN PRIVATE KEY") {
        return RsaPrivateKey::from_pkcs8_pem(trimmed)
            .map_err(|e| anyhow!("invalid alipay PKCS#8 private key: {}", e));
    }
    // Pure base64: try PKCS#8 first, then PKCS#1
    let cleaned = normalize_key_input(trimmed);
    let wrapped = wrap_base64_64(&cleaned);
    let pkcs8_pem = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
        wrapped
    );
    if let Ok(key) = RsaPrivateKey::from_pkcs8_pem(&pkcs8_pem) {
        return Ok(key);
    }
    let pkcs1_pem = format!(
        "-----BEGIN RSA PRIVATE KEY-----\n{}\n-----END RSA PRIVATE KEY-----",
        wrapped
    );
    RsaPrivateKey::from_pkcs1_pem(&pkcs1_pem).map_err(|e| {
        anyhow!(
            "invalid alipay private key (tried PKCS#8 and PKCS#1): {}",
            e
        )
    })
}

fn load_public_key(raw: &str) -> Result<RsaPublicKey> {
    let trimmed = raw.trim();
    if trimmed.contains("BEGIN PUBLIC KEY") {
        return RsaPublicKey::from_public_key_pem(trimmed)
            .map_err(|e| anyhow!("invalid alipay public key: {}", e));
    }
    let cleaned = normalize_key_input(trimmed);
    let wrapped = wrap_base64_64(&cleaned);
    let pem = format!(
        "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----",
        wrapped
    );
    RsaPublicKey::from_public_key_pem(&pem).map_err(|e| anyhow!("invalid alipay public key: {}", e))
}

pub struct AlipayClient {
    config: BillingConfig,
    http: reqwest::Client,
}

impl AlipayClient {
    pub fn new(config: BillingConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    pub fn sign(&self, params: &[(String, String)]) -> Result<String> {
        let mut sorted = params.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let data_to_sign = sorted
            .iter()
            .filter(|(k, _)| k != "sign" && k != "sign_type")
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let priv_key = load_private_key(&self.config.alipay_private_key)?;
        let signing_key = SigningKey::<Sha256>::new(priv_key);
        let signature = signing_key.sign(data_to_sign.as_bytes());
        Ok(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()))
    }

    pub fn verify_signature(
        &self,
        params: &[(String, String)],
        signature_base64: &str,
    ) -> Result<()> {
        let mut sorted = params.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let data_to_sign = sorted
            .iter()
            .filter(|(k, _)| k != "sign" && k != "sign_type")
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let pub_key = load_public_key(&self.config.alipay_public_key)?;
        let verifying_key = VerifyingKey::<Sha256>::new(pub_key);

        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(signature_base64)
            .map_err(|e| anyhow!("invalid signature base64: {}", e))?;
        let sig = rsa::pkcs1v15::Signature::try_from(sig_bytes.as_slice())
            .map_err(|e| anyhow!("invalid signature bytes: {}", e))?;

        verifying_key
            .verify(data_to_sign.as_bytes(), &sig)
            .map_err(|e| anyhow!("alipay signature verification failed: {}", e))
    }

    pub async fn create_precreate_order(
        &self,
        amount: &str,
        subject: &str,
        order_id: &str,
        notify_url: &str,
    ) -> Result<String> {
        if !self.config.alipay_enabled() {
            bail!("alipay_billing_unconfigured");
        }

        let biz_content = serde_json::json!({
            "out_trade_no": order_id,
            "total_amount": amount,
            "subject": subject,
        })
        .to_string();

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let mut params = vec![
            ("app_id".to_string(), self.config.alipay_app_id.clone()),
            ("method".to_string(), "alipay.trade.precreate".to_string()),
            ("charset".to_string(), "utf-8".to_string()),
            ("sign_type".to_string(), "RSA2".to_string()),
            ("timestamp".to_string(), timestamp),
            ("version".to_string(), "1.0".to_string()),
            ("notify_url".to_string(), notify_url.to_string()),
            ("biz_content".to_string(), biz_content),
        ];

        let sign = self.sign(&params)?;
        params.push(("sign".to_string(), sign));

        let response = self
            .http
            .post(&self.config.alipay_gateway_url)
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("alipay_precreate_failed: {body}");
        }

        let json: serde_json::Value = serde_json::from_str(&body)?;
        let resp = json
            .get("alipay_trade_precreate_response")
            .ok_or_else(|| anyhow!("alipay response missing precreate response: {}", body))?;

        let code = resp.get("code").and_then(|c| c.as_str()).unwrap_or("");
        if code != "10000" {
            let sub_msg = resp.get("sub_msg").and_then(|m| m.as_str()).unwrap_or("");
            bail!("alipay precreate failed: {}", sub_msg);
        }

        let qr_code = resp
            .get("qr_code")
            .and_then(|q| q.as_str())
            .ok_or_else(|| anyhow!("alipay response missing qr_code"))?
            .to_string();

        Ok(qr_code)
    }
}
