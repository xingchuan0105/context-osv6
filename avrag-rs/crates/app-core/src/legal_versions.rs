//! Published legal document versions — keep in sync with `frontend_next/lib/legal/versions.ts`.

use common::AppError;

pub const PUBLISHED_TERMS_VERSION: &str = "2026-06-13";
pub const PUBLISHED_PRIVACY_VERSION: &str = "2026-06-13";

pub fn validate_published_legal_versions(
    terms_version: &str,
    privacy_version: &str,
) -> Result<(), AppError> {
    let terms = terms_version.trim();
    let privacy = privacy_version.trim();
    if terms != PUBLISHED_TERMS_VERSION {
        return Err(AppError::validation(
            "invalid_terms_version",
            "Terms version does not match the currently published agreement",
        ));
    }
    if privacy != PUBLISHED_PRIVACY_VERSION {
        return Err(AppError::validation(
            "invalid_privacy_version",
            "Privacy version does not match the currently published policy",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_published_versions() {
        assert!(
            validate_published_legal_versions(PUBLISHED_TERMS_VERSION, PUBLISHED_PRIVACY_VERSION)
                .is_ok()
        );
    }

    #[test]
    fn rejects_stale_terms_version() {
        let err = validate_published_legal_versions("2025-01-01", PUBLISHED_PRIVACY_VERSION)
            .expect_err("stale terms");
        assert_eq!(err.code(), "invalid_terms_version");
    }
}
