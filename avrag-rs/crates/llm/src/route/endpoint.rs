use crate::schema::LlmError;

#[derive(Debug, Clone, Default)]
pub struct Endpoint {
    pub base_url: Option<String>,
    pub path: String,
    pub query: Vec<(String, String)>,
}

impl Endpoint {
    pub fn new(base_url: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            base_url: Some(base_url.into()),
            path: path.into(),
            query: Vec::new(),
        }
    }

    pub fn render(&self) -> Result<String, LlmError> {
        let base = self
            .base_url
            .as_deref()
            .ok_or_else(|| LlmError::config("endpoint base_url is required"))?
            .trim_end_matches('/');
        let path = if self.path.starts_with('/') {
            self.path.clone()
        } else {
            format!("/{}", self.path)
        };
        let mut url = format!("{base}{path}");
        if !self.query.is_empty() {
            let query = self
                .query
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding(k), urlencoding(v)))
                .collect::<Vec<_>>()
                .join("&");
            url.push('?');
            url.push_str(&query);
        }
        Ok(url)
    }

    pub fn merge(mut self, other: &Endpoint) -> Self {
        if other.base_url.is_some() {
            self.base_url = other.base_url.clone();
        }
        if !other.path.is_empty() {
            self.path = other.path.clone();
        }
        if !other.query.is_empty() {
            self.query.extend(other.query.iter().cloned());
        }
        self
    }
}

fn urlencoding(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Endpoint;

    #[test]
    fn render_joins_base_and_path() {
        let endpoint = Endpoint::new("https://api.example.com/v1", "/chat/completions");
        assert_eq!(
            endpoint.render().unwrap(),
            "https://api.example.com/v1/chat/completions"
        );
    }
}
