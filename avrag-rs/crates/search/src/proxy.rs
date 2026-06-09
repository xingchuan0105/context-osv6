use reqwest::Client;

/// Read proxy URL from env (`HTTPS_PROXY` / `HTTP_PROXY` and lowercase variants),
/// rewriting `127.0.0.1` / `localhost` to the WSL Windows host when applicable.
pub fn resolved_proxy_url() -> Option<String> {
    std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .or_else(|_| std::env::var("HTTP_PROXY"))
        .or_else(|_| std::env::var("http_proxy"))
        .ok()
        .map(|url| rewrite_localhost_proxy_for_wsl(&url))
}

/// Rewrite proxy env vars in-process so child processes inherit the WSL-safe URL.
pub fn sync_resolved_proxy_env() {
    let Some(resolved) = resolved_proxy_url() else {
        return;
    };
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        match std::env::var(key) {
            Ok(current) if current == resolved => {}
            Ok(current) if current.contains("127.0.0.1") || current.contains("localhost") => {
                unsafe { std::env::set_var(key, &resolved) };
            }
            Err(_) => {}
            Ok(_) => {}
        }
    }
}

/// Build a reqwest client that routes through the resolved proxy when configured.
pub fn build_http_client_with_proxy() -> Client {
    let mut builder = Client::builder();
    if let Some(proxy_url) = resolved_proxy_url() {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
        }
    }
    builder.build().unwrap_or_else(|_| Client::new())
}

/// On WSL/Linux, `127.0.0.1` in proxy URLs points at the Linux namespace, not
/// the Windows host where the VPN client listens. Use the default-route gateway.
fn rewrite_localhost_proxy_for_wsl(proxy_url: &str) -> String {
    if !proxy_url.contains("127.0.0.1") && !proxy_url.contains("localhost") {
        return proxy_url.to_string();
    }
    let Some(gw) = linux_default_gateway() else {
        return proxy_url.to_string();
    };
    proxy_url
        .replace("127.0.0.1", &gw)
        .replace("localhost", &gw)
}

fn linux_default_gateway() -> Option<String> {
    let content = std::fs::read_to_string("/proc/net/route").ok()?;
    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 || fields[1] != "00000000" {
            continue;
        }
        let hex = fields[2];
        if hex.len() != 8 {
            continue;
        }
        let parse = |s: &str| u8::from_str_radix(s, 16).ok();
        let a = parse(&hex[6..8])?;
        let b = parse(&hex[4..6])?;
        let c = parse(&hex[2..4])?;
        let d = parse(&hex[0..2])?;
        if a == 0 && b == 0 && c == 0 && d == 0 {
            continue;
        }
        return Some(format!("{a}.{b}.{c}.{d}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_localhost_proxy_uses_gateway_when_available() {
        if linux_default_gateway().is_none() {
            return;
        }
        let gw = linux_default_gateway().unwrap();
        assert_eq!(
            rewrite_localhost_proxy_for_wsl("http://127.0.0.1:20000"),
            format!("http://{gw}:20000")
        );
    }
}
