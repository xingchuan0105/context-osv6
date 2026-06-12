use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::str::FromStr;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SsrfError {
    #[error("invalid url")]
    InvalidUrl,
    #[error("url scheme not allowed: only http and https are permitted")]
    InvalidScheme,
    #[error("url host is not permitted")]
    BlockedHost,
    #[error("url host dns resolution failed")]
    DnsResolutionFailed,
}

pub fn validate_http_url(url: &str) -> Result<(), SsrfError> {
    validate_http_url_with_dns(url, false)
}

pub fn validate_http_url_with_dns(url: &str, resolve_dns: bool) -> Result<(), SsrfError> {
    let parsed = url::Url::parse(url).map_err(|_| SsrfError::InvalidUrl)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(SsrfError::InvalidScheme);
    }
    let host = parsed
        .host_str()
        .ok_or(SsrfError::BlockedHost)?
        .trim()
        .trim_end_matches('.');
    if host.is_empty() {
        return Err(SsrfError::BlockedHost);
    }
    validate_host(host, parsed.port().unwrap_or(443), resolve_dns)
}

fn validate_host(host: &str, port: u16, resolve_dns: bool) -> Result<(), SsrfError> {
    let normalized = host.to_ascii_lowercase();
    if normalized == "localhost" || normalized.ends_with(".localhost") {
        return Err(SsrfError::BlockedHost);
    }

    let host_for_ip = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = IpAddr::from_str(host_for_ip) {
        if is_blocked_ip(ip) {
            return Err(SsrfError::BlockedHost);
        }
        return Ok(());
    }

    if !resolve_dns {
        return Ok(());
    }

    let endpoint = format!("{host}:{port}");
    let addresses = endpoint
        .to_socket_addrs()
        .map_err(|_| SsrfError::DnsResolutionFailed)?;
    let mut saw_address = false;
    for address in addresses {
        saw_address = true;
        if is_blocked_ip(address.ip()) {
            return Err(SsrfError::BlockedHost);
        }
    }
    if saw_address {
        Ok(())
    } else {
        Err(SsrfError::DnsResolutionFailed)
    }
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_blocked_ipv4(ipv4),
        IpAddr::V6(ipv6) => is_blocked_ipv6(ipv6),
    }
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.octets()[0] == 0
        || metadata_ipv4(ip)
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || is_unicast_link_local_ipv6(ip)
        || is_unique_local_ipv6(ip)
}

fn metadata_ipv4(ip: Ipv4Addr) -> bool {
    ip.octets()[0] == 169 && ip.octets()[1] == 254
}

fn is_unicast_link_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_unique_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_public_http_url() {
        assert!(validate_http_url("http://example.com/doc").is_ok());
        assert!(validate_http_url("https://example.com/doc").is_ok());
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert_eq!(
            validate_http_url("file:///etc/passwd"),
            Err(SsrfError::InvalidScheme)
        );
        assert_eq!(
            validate_http_url("ftp://example.com"),
            Err(SsrfError::InvalidScheme)
        );
    }

    #[test]
    fn rejects_loopback_ip_literal() {
        assert_eq!(
            validate_http_url("http://127.0.0.1/"),
            Err(SsrfError::BlockedHost)
        );
        assert_eq!(
            validate_http_url("http://[::1]/"),
            Err(SsrfError::BlockedHost)
        );
    }

    #[test]
    fn rejects_private_ip_literals() {
        assert_eq!(
            validate_http_url("http://10.0.0.1/"),
            Err(SsrfError::BlockedHost)
        );
        assert_eq!(
            validate_http_url("http://192.168.1.1/"),
            Err(SsrfError::BlockedHost)
        );
        assert_eq!(
            validate_http_url("http://172.16.0.1/"),
            Err(SsrfError::BlockedHost)
        );
    }

    #[test]
    fn rejects_link_local_and_metadata_ips() {
        assert_eq!(
            validate_http_url("http://169.254.169.254/"),
            Err(SsrfError::BlockedHost)
        );
        assert_eq!(
            validate_http_url("http://169.254.1.10/"),
            Err(SsrfError::BlockedHost)
        );
    }

    #[test]
    fn rejects_localhost_hostname() {
        assert_eq!(
            validate_http_url("http://localhost/"),
            Err(SsrfError::BlockedHost)
        );
    }

    #[test]
    fn skips_dns_resolution_when_disabled() {
        assert!(validate_http_url_with_dns("http://metadata.internal/", false).is_ok());
    }
}
