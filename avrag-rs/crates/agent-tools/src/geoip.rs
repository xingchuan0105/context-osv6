//! MaxMind GeoLite2 city lookup for `user_context`.
//!
//! DB path: `GEOIP_CITY_DB_PATH`. Missing DB or private IPs degrade cleanly
//! without inventing a city.

use std::net::IpAddr;
use std::path::Path;
use std::sync::OnceLock;

use maxminddb::geoip2;
use maxminddb::Reader;

static READER: OnceLock<Option<Reader<Vec<u8>>>> = OnceLock::new();

#[derive(Debug, Clone, Default)]
pub struct GeoCity {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Clone)]
pub enum GeoLookupError {
    PrivateOrLocal,
    InvalidIp,
    DbUnavailable,
    NotFound,
}

impl GeoLookupError {
    pub fn as_reason(&self) -> &'static str {
        match self {
            Self::PrivateOrLocal => "private_or_local_ip",
            Self::InvalidIp => "invalid_ip",
            Self::DbUnavailable => "geoip_db_unavailable",
            Self::NotFound => "geo_not_found",
        }
    }
}

fn reader() -> Option<&'static Reader<Vec<u8>>> {
    READER
        .get_or_init(|| {
            let path = std::env::var("GEOIP_CITY_DB_PATH").ok()?;
            let p = Path::new(&path);
            if !p.exists() {
                tracing::warn!(path = %path, "GEOIP_CITY_DB_PATH set but file missing");
                return None;
            }
            match Reader::open_readfile(p) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::warn!(error = %e, path = %path, "failed to open GeoLite2 DB");
                    None
                }
            }
        })
        .as_ref()
}

fn is_unusable_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local(),
    }
}

/// Look up city-level geo for a client IP string.
pub fn lookup_city(ip: &str) -> Result<GeoCity, GeoLookupError> {
    let trimmed = ip.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        return Err(GeoLookupError::InvalidIp);
    }
    let addr: IpAddr = trimmed.parse().map_err(|_| GeoLookupError::InvalidIp)?;
    if is_unusable_ip(addr) {
        return Err(GeoLookupError::PrivateOrLocal);
    }
    let Some(db) = reader() else {
        return Err(GeoLookupError::DbUnavailable);
    };
    let city: geoip2::City = db.lookup(addr).map_err(|_| GeoLookupError::NotFound)?;

    let country = city
        .country
        .as_ref()
        .and_then(|c| c.iso_code.map(|s| s.to_string()));
    let region = city.subdivisions.as_ref().and_then(|subs| {
        subs.first().and_then(|s| {
            s.names
                .as_ref()
                .and_then(|n| n.get("en").map(|v| (*v).to_string()))
                .or_else(|| s.iso_code.map(|c| c.to_string()))
        })
    });
    let city_name = city.city.as_ref().and_then(|c| {
        c.names
            .as_ref()
            .and_then(|n| n.get("en").map(|v| (*v).to_string()))
    });

    if country.is_none() && region.is_none() && city_name.is_none() {
        return Err(GeoLookupError::NotFound);
    }

    Ok(GeoCity {
        country,
        region,
        city: city_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ip_no_city() {
        let err = lookup_city("127.0.0.1").unwrap_err();
        assert!(matches!(err, GeoLookupError::PrivateOrLocal));
        assert_eq!(err.as_reason(), "private_or_local_ip");
    }

    #[test]
    fn invalid_ip() {
        let err = lookup_city("not-an-ip").unwrap_err();
        assert!(matches!(err, GeoLookupError::InvalidIp));
    }

    #[test]
    fn unknown_token() {
        let err = lookup_city("unknown").unwrap_err();
        assert!(matches!(err, GeoLookupError::InvalidIp));
    }
}
