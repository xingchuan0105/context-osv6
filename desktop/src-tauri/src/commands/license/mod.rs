//! Desktop licensing: types, offline file/status service, and Tauri commands.
mod commands;
mod service;
mod types;

pub use commands::*;
pub use service::handle_deep_link_url;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::service::{
        build_dev_certificate, compute_device_id, parse_activate_key, resolve_license_status,
    };
    use super::types::*;

    #[test]
    fn get_device_id_is_stable_within_process() {
        let first = match compute_device_id() {
            Ok(id) => id,
            Err(_) => {
                // WSL/CI environments may not expose drive serial metadata.
                return;
            }
        };
        let second = compute_device_id().expect("device id should succeed after first success");
        assert_eq!(first, second);
        assert!(!first.is_empty());
    }

    #[test]
    fn parse_activate_key_extracts_license_key() {
        let key = parse_activate_key("avrag-desktop://activate?key=AVRG-TEST-KEY").expect("key");
        assert_eq!(key, "AVRG-TEST-KEY");
    }

    #[test]
    fn resolve_status_unactivated_without_file() {
        let status = resolve_license_status(None, "device-a", 1_700_000_000, true);
        assert_eq!(status.kind, LicenseStatusKind::Unactivated);
    }

    #[test]
    fn resolve_status_trial_with_remaining_days() {
        let now = 1_700_000_000_i64;
        let file = LicenseFile {
            key: "trial-key".to_string(),
            license_id: "lic-1".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", Some(now + 86_400), 1),
            kind: LicenseKind::Trial,
            issued_at: now,
            expires_at: Some(now + 86_400),
            last_heartbeat: Some(now),
            revoked: false,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::Trial);
        assert_eq!(status.days_remaining, Some(1));
    }

    #[test]
    fn resolve_status_expired_after_offline_grace() {
        let now = 1_700_000_000_i64;
        let expired_at = now - 10;
        let file = LicenseFile {
            key: "paid-key".to_string(),
            license_id: "lic-2".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", Some(expired_at), 1),
            kind: LicenseKind::Standard,
            issued_at: now - OFFLINE_GRACE_SECS - 10,
            expires_at: Some(expired_at),
            last_heartbeat: Some(now - OFFLINE_GRACE_SECS - 10),
            revoked: false,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::Expired);
    }

    #[test]
    fn resolve_status_offline_grace_before_expiry_window_ends() {
        let now = 1_700_000_000_i64;
        let expired_at = now - 10;
        let last_heartbeat = now - 86_400;
        let file = LicenseFile {
            key: "paid-key".to_string(),
            license_id: "lic-3".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", Some(expired_at), 1),
            kind: LicenseKind::Standard,
            issued_at: now - 86_400,
            expires_at: Some(expired_at),
            last_heartbeat: Some(last_heartbeat),
            revoked: false,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::OfflineGrace);
        assert!(status.offline_grace_days.unwrap_or(0) > 0);
    }

    #[test]
    fn resolve_status_revoked_flag() {
        let now = 1_700_000_000_i64;
        let file = LicenseFile {
            key: "paid-key".to_string(),
            license_id: "lic-4".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", None, 1),
            kind: LicenseKind::Pro,
            issued_at: now,
            expires_at: None,
            last_heartbeat: Some(now),
            revoked: true,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::Revoked);
    }
}
