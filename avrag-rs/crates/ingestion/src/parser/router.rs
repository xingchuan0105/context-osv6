use anyhow::Result;

use super::probe::{ParseProbe, ParseProbeResult};

#[derive(Debug, Clone, PartialEq)]
pub enum ParseRoute {
    Local,
    MineruPrecise,
}

#[derive(Debug, Clone)]
pub enum RouteReason {
    TextFile,
    ImageFile,
    PresentationFile,
    SimplePdf,
    ComplexPdf,
    ScannedPdf,
    Default,
}

impl std::fmt::Display for RouteReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteReason::TextFile => write!(f, "text_file"),
            RouteReason::ImageFile => write!(f, "image_file"),
            RouteReason::PresentationFile => write!(f, "presentation_file"),
            RouteReason::SimplePdf => write!(f, "simple_pdf"),
            RouteReason::ComplexPdf => write!(f, "complex_pdf"),
            RouteReason::ScannedPdf => write!(f, "scanned_pdf"),
            RouteReason::Default => write!(f, "default"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseRouteDecision {
    pub route: ParseRoute,
    pub reason: RouteReason,
    pub probe_result: Option<ParseProbeResult>,
}

pub struct ParseRouter;

impl ParseRouter {
    pub fn route(bytes: &[u8], filename: &str) -> Result<ParseRouteDecision> {
        let extension = filename
            .rsplit('.')
            .next()
            .unwrap_or("unknown")
            .to_lowercase();

        match extension.as_str() {
            "txt" | "md" | "rst" | "csv" | "json" | "toml" | "yaml" | "yml" => {
                Ok(ParseRouteDecision {
                    route: ParseRoute::Local,
                    reason: RouteReason::TextFile,
                    probe_result: None,
                })
            }
            "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" | "rb" | "php" => {
                Ok(ParseRouteDecision {
                    route: ParseRoute::Local,
                    reason: RouteReason::TextFile,
                    probe_result: None,
                })
            }
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Ok(ParseRouteDecision {
                route: ParseRoute::MineruPrecise,
                reason: RouteReason::ImageFile,
                probe_result: None,
            }),
            "ppt" | "pptx" => Ok(ParseRouteDecision {
                route: ParseRoute::MineruPrecise,
                reason: RouteReason::PresentationFile,
                probe_result: None,
            }),
            "pdf" => {
                let probe_result = ParseProbe::probe(bytes, filename)?;
                let route = if probe_result.likely_scanned {
                    ParseRoute::MineruPrecise
                } else if probe_result.image_hint_count > 5 || probe_result.table_hint_count > 10 {
                    ParseRoute::MineruPrecise
                } else {
                    ParseRoute::Local
                };
                let reason = if probe_result.likely_scanned {
                    RouteReason::ScannedPdf
                } else if probe_result.image_hint_count > 5 || probe_result.table_hint_count > 10 {
                    RouteReason::ComplexPdf
                } else {
                    RouteReason::SimplePdf
                };
                Ok(ParseRouteDecision {
                    route,
                    reason,
                    probe_result: Some(probe_result),
                })
            }
            _ => Ok(ParseRouteDecision {
                route: ParseRoute::Local,
                reason: RouteReason::Default,
                probe_result: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_file_routing() {
        let decision = ParseRouter::route(b"hello world", "test.txt").unwrap();
        assert_eq!(decision.route, ParseRoute::Local);
        assert!(matches!(decision.reason, RouteReason::TextFile));
    }

    #[test]
    fn test_image_file_routing() {
        let decision = ParseRouter::route(b"fake image", "test.png").unwrap();
        assert_eq!(decision.route, ParseRoute::MineruPrecise);
        assert!(matches!(decision.reason, RouteReason::ImageFile));
    }

    #[test]
    fn test_presentation_file_routing() {
        let decision = ParseRouter::route(b"fake ppt", "test.pptx").unwrap();
        assert_eq!(decision.route, ParseRoute::MineruPrecise);
        assert!(matches!(decision.reason, RouteReason::PresentationFile));
    }
}
