//! PII scrubbing guard.
//!
//! Detects and redacts personally identifiable information:
//! - Social Security Numbers
//! - Credit card numbers
//! - Email addresses
//! - Phone numbers
//! - Medical record numbers

use contracts::chat::GuardResult;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

lazy_static! {
    /// PII patterns: (regex, replacement, label)
    static ref PII_PATTERNS: Vec<(Regex, &'static str, &'static str)> = vec![
        // SSN: 123-45-6789 (with dashes)
        (
            Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(),
            "[SSN_REDACTED]",
            "ssn",
        ),
        // Credit card: 16 digits with optional separators
        (
            Regex::new(r"\b(?:\d{4}[-\s]?){3}\d{4}\b").unwrap(),
            "[CARD_REDACTED]",
            "credit_card",
        ),
        // Email address
        (
            Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap(),
            "[EMAIL_REDACTED]",
            "email",
        ),
        // US phone: (123) 456-7890 or 123-456-7890 or 1234567890
        (
            Regex::new(r"\b(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap(),
            "[PHONE_REDACTED]",
            "phone",
        ),
        // Medical record number (MRN) — pattern: 7-10 digits
        (
            Regex::new(r"\bMRN[:\s]?\d{7,10}\b").unwrap(),
            "[MRN_REDACTED]",
            "mrn",
        ),
        // Passport number — 8-9 alphanumeric
        (
            Regex::new(r"\b[A-Z]{1,2}\d{6,9}\b").unwrap(),
            "[PASSPORT_REDACTED]",
            "passport",
        ),
        // Driver's license — varying patterns, common: 1-2 letters + 5-8 digits
        (
            Regex::new(r"\b[A-Z]{1,2}\d{5,8}\b").unwrap(),
            "[DL_REDACTED]",
            "drivers_license",
        ),
    ];
}

/// Guard that detects and redacts PII from synthesizer output.
#[derive(Debug, Clone)]
pub struct PiiScrubberGuard;

impl PiiScrubberGuard {
    pub fn new() -> Self {
        Self
    }

    /// Scan the response for PII. Returns a `GuardResult` with redaction details.
    /// Note: this guard does NOT modify the response string — the caller must
    /// apply the redactions using `scrub()`.
    pub fn check(&self, response: &str, _trace_id: Option<String>) -> GuardResult {
        let mut redaction_counts: HashMap<&str, u64> = HashMap::new();
        let mut total_redactions = 0u64;

        for (re, _, label) in PII_PATTERNS.iter() {
            let count = re.find_iter(response).count() as u64;
            if count > 0 {
                *redaction_counts.entry(label).or_insert(0) += count;
                total_redactions += count;
            }
        }

        if total_redactions == 0 {
            return GuardResult::pass("output:pii_detection");
        }

        GuardResult::redact(
            "output:pii_detection",
            format!("Detected {} PII instances", total_redactions),
            serde_json::json!({
                "redacted_count": total_redactions,
                "breakdown": redaction_counts,
            }),
        )
    }

    /// Apply redactions to the response string, returning the sanitized version.
    pub fn scrub(&self, response: &str) -> String {
        let mut scrubbed = response.to_string();
        for (re, replacement, _) in PII_PATTERNS.iter() {
            scrubbed = re.replace_all(&scrubbed, *replacement).to_string();
        }
        scrubbed
    }
}

impl Default for PiiScrubberGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_pii_passed() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check("Hello world, this is a normal response.", None);
        assert!(result.passed);
    }

    #[test]
    fn test_ssn_detected() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check("My SSN is 123-45-6789", None);
        assert!(result.passed); // PII guard redacts, not blocks
        assert_eq!(result.action, contracts::chat::GuardAction::Redact);
        let details = result.details.as_ref().unwrap();
        assert_eq!(details["redacted_count"], 1);
    }

    #[test]
    fn test_email_detected() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check("Contact me at john.doe@example.com", None);
        assert!(result.passed);
        let details = result.details.as_ref().unwrap();
        assert!(details["breakdown"]["email"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_scrub_applies_replacements() {
        let guard = PiiScrubberGuard::new();
        let input = "SSN: 123-45-6789, Email: test@example.com";
        let scrubbed = guard.scrub(input);
        assert!(scrubbed.contains("[SSN_REDACTED]"));
        assert!(scrubbed.contains("[EMAIL_REDACTED]"));
        assert!(!scrubbed.contains("123-45-6789"));
        assert!(!scrubbed.contains("test@example.com"));
    }

    #[test]
    fn test_credit_card_detected() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check("Card: 4111-1111-1111-1111", None);
        assert!(result.passed);
        assert_eq!(result.action, contracts::chat::GuardAction::Redact);
    }

    #[test]
    fn test_phone_number_detected() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check("Call me at (555) 123-4567", None);
        assert!(result.passed);
        let details = result.details.as_ref().unwrap();
        assert!(details["breakdown"]["phone"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_mrn_detected() {
        let guard = PiiScrubberGuard::new();
        // MRN followed directly by 7-10 digits (no colon-space between)
        let result = guard.check("Patient MRN12345678", None);
        assert!(result.passed);
        let details = result.details.as_ref().unwrap();
        assert!(details["breakdown"]["mrn"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_passport_detected() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check("Passport: A123456789", None);
        assert!(result.passed);
        let details = result.details.as_ref().unwrap();
        assert!(details["breakdown"]["passport"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_drivers_license_detected() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check("License: D12345678", None);
        assert!(result.passed);
        let details = result.details.as_ref().unwrap();
        assert!(details["breakdown"]["drivers_license"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_multiple_pii_types_detected() {
        let guard = PiiScrubberGuard::new();
        let result = guard.check(
            "SSN: 123-45-6789, Email: alice@example.com, Phone: 555-987-6543",
            None,
        );
        assert!(result.passed);
        let details = result.details.as_ref().unwrap();
        assert_eq!(details["redacted_count"], 3);
    }

    #[test]
    fn test_scrub_preserves_non_pii_text() {
        let guard = PiiScrubberGuard::new();
        let input = "Hello, the capital of France is Paris.";
        let scrubbed = guard.scrub(input);
        assert_eq!(scrubbed, input);
    }
}
