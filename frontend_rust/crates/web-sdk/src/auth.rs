use serde::{Deserialize, Serialize};

use crate::conversation_api::trim_base_url;

/// Next `lib/auth/context.tsx` / `server-session.ts` 同一套键与 cookie 名。
pub const AUTH_STORAGE_KEY: &str = "avrag.auth.v1";
pub const AUTH_SESSION_COOKIE_NAME: &str = "avrag.auth.session";
pub const AUTH_SESSION_COOKIE_VALUE: &str = "1";
pub const AUTH_PERSISTED_COOKIE_NAME: &str = "avrag.auth.persisted";
pub const AUTH_SESSION_COOKIE_MAX_AGE: u64 = 60 * 60 * 24 * 365;
pub const AUTH_BOOTSTRAP_TIMEOUT_MS: i32 = 3000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub full_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_profile_enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedAuth {
    pub token: String,
    pub user: AuthUser,
}

#[derive(Debug, Deserialize)]
struct AuthMeEnvelope {
    success: bool,
    data: Option<AuthMeData>,
}

#[derive(Debug, Deserialize)]
struct AuthMeData {
    user: AuthUser,
}

pub fn auth_me_url(base_url: &str) -> String {
    format!("{}/api/auth/me", trim_base_url(base_url))
}

pub fn parse_persisted_auth_json(raw: &str) -> Option<PersistedAuth> {
    let parsed: PersistedAuth = serde_json::from_str(raw).ok()?;
    if parsed.token.is_empty() || parsed.user.id.is_empty() {
        return None;
    }
    Some(parsed)
}

pub fn parse_cookie_value<'a>(cookie_header: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    cookie_header.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(prefix.as_str())
    })
}

pub fn parse_persisted_auth_cookie(cookie_header: &str) -> Option<PersistedAuth> {
    let raw = parse_cookie_value(cookie_header, AUTH_PERSISTED_COOKIE_NAME)?;
    if raw.is_empty() {
        return None;
    }
    let decoded = decode_uri_component(raw)?;
    parse_persisted_auth_json(&decoded)
}

pub fn has_auth_session_hint(cookie_header: &str) -> bool {
    parse_cookie_value(cookie_header, AUTH_SESSION_COOKIE_NAME) == Some(AUTH_SESSION_COOKIE_VALUE)
}

pub fn cookie_security_attrs(secure: bool) -> String {
    if secure {
        "; Path=/; SameSite=Lax; Secure".to_string()
    } else {
        "; Path=/; SameSite=Lax".to_string()
    }
}

pub fn session_hint_set_cookie(secure: bool) -> String {
    format!(
        "{AUTH_SESSION_COOKIE_NAME}={AUTH_SESSION_COOKIE_VALUE}{}; Max-Age={AUTH_SESSION_COOKIE_MAX_AGE}",
        cookie_security_attrs(secure)
    )
}

pub fn session_hint_clear_cookie(secure: bool) -> String {
    format!(
        "{AUTH_SESSION_COOKIE_NAME}={AUTH_SESSION_COOKIE_VALUE}{}; Max-Age=0",
        cookie_security_attrs(secure)
    )
}

pub fn persisted_set_cookie(auth: &PersistedAuth, secure: bool) -> String {
    let payload = encode_uri_component(&serde_json::to_string(auth).expect("PersistedAuth json"));
    format!(
        "{AUTH_PERSISTED_COOKIE_NAME}={payload}{}; Max-Age={AUTH_SESSION_COOKIE_MAX_AGE}",
        cookie_security_attrs(secure)
    )
}

pub fn persisted_clear_cookie(secure: bool) -> String {
    format!(
        "{AUTH_PERSISTED_COOKIE_NAME}={}; Max-Age=0",
        cookie_security_attrs(secure)
    )
}

pub fn parse_auth_me_json(body: &[u8]) -> Option<AuthUser> {
    let envelope: AuthMeEnvelope = serde_json::from_slice(body).ok()?;
    if !envelope.success {
        return None;
    }
    Some(envelope.data?.user)
}

/// `encodeURIComponent` 子集：未保留字节写成 `%HH`。
pub fn encode_uri_component(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn decode_uri_component(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}
