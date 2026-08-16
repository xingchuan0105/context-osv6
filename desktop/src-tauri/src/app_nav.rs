//! In-app vs OS-browser URL rules for the packaged WebView.

use url::Url;

const TAURI_HOSTS: &[&str] = &["tauri.localhost", "ipc.localhost"];

pub fn is_webview_local_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    parsed
        .host_str()
        .is_some_and(|h| TAURI_HOSTS.iter().any(|known| *known == h))
}

/// `/dashboard/{id}` → `/dashboard/_placeholder?ws={id}` (static export file).
pub fn map_static_export_href(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    let path = parsed.path().to_string();
    let mut parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[1] == "dashboard" {
        let seg = parts[2];
        let (base, suffix) = strip_known_ext(seg);
        if !is_reserved_dashboard(base) {
            let placeholder = format!("_placeholder{suffix}");
            parts[2] = placeholder.as_str();
            let new_path = parts.join("/");
            let mut pairs: Vec<(String, String)> = parsed
                .query_pairs()
                .filter(|(k, _)| k != "ws")
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            pairs.insert(0, ("ws".into(), base.to_string()));
            parsed.set_path(&new_path);
            parsed.set_query(None);
            {
                let mut query = parsed.query_pairs_mut();
                for (k, v) in &pairs {
                    query.append_pair(k, v);
                }
            }
            return parsed.to_string();
        }
    }
    url.to_string()
}

fn strip_known_ext(segment: &str) -> (&str, &str) {
    if let Some(base) = segment.strip_suffix(".html") {
        return (base, ".html");
    }
    if let Some(base) = segment.strip_suffix(".txt") {
        return (base, ".txt");
    }
    (segment, "")
}

fn is_reserved_dashboard(base: &str) -> bool {
    base == "analytics" || base == "_placeholder" || base.starts_with("__next")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_localhost_is_in_webview() {
        assert!(is_webview_local_url(
            "http://tauri.localhost/dashboard/67501731-e2c2-4d3f-ae71-7bdb711368e8"
        ));
        assert!(!is_webview_local_url("https://context-os.com/pricing"));
    }

    #[test]
    fn maps_workspace_path_to_placeholder() {
        let out = map_static_export_href(
            "http://tauri.localhost/dashboard/67501731-e2c2-4d3f-ae71-7bdb711368e8",
        );
        assert!(out.contains("/dashboard/_placeholder"));
        assert!(out.contains("ws=67501731-e2c2-4d3f-ae71-7bdb711368e8"));
    }
}
