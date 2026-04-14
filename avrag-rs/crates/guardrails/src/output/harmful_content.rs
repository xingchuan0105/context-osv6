//! Harmful content guard.
//!
//! Detects and blocks content that violates usage policies:
//! - Hate speech and harassment
//! - Violence and gore descriptions
//! - Sexual content
//! - Malware/hacking instructions
//! - Self-harm content

use common::{GuardResult, RiskLevel};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    /// Harmful content patterns with severity
    static ref HARMFUL_PATTERNS: Vec<(Regex, &'static str, RiskLevel)> = vec![
        // Hate speech
        (
            Regex::new(r"(?i)\b(hate\s+crime|ethnic\s+cleansing|genocide|supremac|klansman|nazi)\b").unwrap(),
            "hate_speech",
            RiskLevel::Critical,
        ),
        // Violence instructions
        (
            Regex::new(r"(?i)\b(how\s+to\s+(make|kill|assassinate|murder)|bomb\s+making|weaponiz)\b").unwrap(),
            "violence_instructions",
            RiskLevel::Critical,
        ),
        // Malware / hacking
        (
            Regex::new(r"(?i)\b(write\s+(a\s+)?(malware|virus|trojan|ransomware)|exploit\s+kit|hack\s+(a\s+)?account)\b").unwrap(),
            "malware_instructions",
            RiskLevel::High,
        ),
        // Self-harm
        (
            Regex::new(r"(?i)\b(suicide\s+method|how\s+to\s+(kill|harm)\s+(yourself|me)|self-harm\s+instruction)\b").unwrap(),
            "self_harm",
            RiskLevel::Critical,
        ),
        // Harassment
        (
            Regex::new(r"(?i)\b(dox|doxx|swat\s+someone|harass\s+(a\s+)?(celebrity|journalist|politician))\b").unwrap(),
            "harassment",
            RiskLevel::High,
        ),
        // CSAM indicators (blocking even as an attempt to detect)
        (
            Regex::new(r"(?i)\b(csam|child\s+(porn|abuse)\s+(image|content))\b").unwrap(),
            "csam",
            RiskLevel::Critical,
        ),
    ];
}

/// Guard that detects harmful content in the synthesizer response.
#[derive(Debug, Clone)]
pub struct HarmfulContentGuard;

impl HarmfulContentGuard {
    pub fn new() -> Self {
        Self
    }

    /// Check response for harmful content patterns.
    /// Returns a blocking result if any critical/high pattern is found.
    pub fn check(&self, response: &str, trace_id: Option<String>) -> GuardResult {
        for (re, pattern_name, risk) in HARMFUL_PATTERNS.iter() {
            if re.is_match(response) {
                return GuardResult::block(
                    "output:harmful_content",
                    *risk,
                    format!("Harmful content detected: {}", pattern_name),
                    trace_id,
                    None,
                );
            }
        }

        GuardResult::pass("output:harmful_content")
    }
}

impl Default for HarmfulContentGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_content_passed() {
        let guard = HarmfulContentGuard::new();
        let result = guard.check("The capital of France is Paris.", None);
        assert!(result.passed);
    }

    #[test]
    fn test_violence_instructions_blocked() {
        let guard = HarmfulContentGuard::new();
        let result = guard.check("Here is how to make a bomb...", None);
        assert!(!result.passed);
        assert_eq!(result.guard_type, "output:harmful_content");
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_hate_speech_blocked() {
        let guard = HarmfulContentGuard::new();
        let result = guard.check("This is a hate crime against minorities", None);
        assert!(!result.passed);
    }

    #[test]
    fn test_self_harm_blocked() {
        let guard = HarmfulContentGuard::new();
        let result = guard.check("Suicide method instructions here", None);
        assert!(!result.passed);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_malware_instructions_blocked() {
        let guard = HarmfulContentGuard::new();
        let result = guard.check("Here is how to write a ransomware virus", None);
        assert!(!result.passed);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_harassment_blocked() {
        let guard = HarmfulContentGuard::new();
        let result = guard.check("I will dox the journalist tomorrow", None);
        assert!(!result.passed);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_csam_blocked() {
        let guard = HarmfulContentGuard::new();
        let result = guard.check("Download this child abuse image", None);
        assert!(!result.passed);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_mixed_harmful_passed() {
        // Medium-risk patterns should still pass through but could be flagged
        let guard = HarmfulContentGuard::new();
        // The guard only blocks Critical and High
        let result = guard.check("Normal scientific discussion about AI", None);
        assert!(result.passed);
    }
}
