pub struct PricingRevampFlag {
    pub rollout_percentage: u8,
}

impl PricingRevampFlag {
    pub fn from_env() -> Self {
        Self {
            rollout_percentage: std::env::var("PRICING_REVAMP_ROLLOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        }
    }

    pub fn is_enabled_for(&self, user_id: uuid::Uuid) -> bool {
        if self.rollout_percentage >= 100 {
            return true;
        }
        if self.rollout_percentage == 0 {
            return false;
        }
        let bucket = (user_id.as_u128() % 100) as u8;
        bucket < self.rollout_percentage
    }
}

#[cfg(test)]
mod tests {
    use super::PricingRevampFlag;

    #[test]
    fn rollout_zero_disables_all_users() {
        let flag = PricingRevampFlag {
            rollout_percentage: 0,
        };
        assert!(!flag.is_enabled_for(uuid::Uuid::new_v4()));
    }

    #[test]
    fn rollout_100_enables_all_users() {
        let flag = PricingRevampFlag {
            rollout_percentage: 100,
        };
        assert!(flag.is_enabled_for(uuid::Uuid::new_v4()));
    }

    #[test]
    fn from_env_unset_defaults_to_disabled() {
        // SAFETY: integration-test binary; no parallel env mutation in this file.
        unsafe {
            std::env::remove_var("PRICING_REVAMP_ROLLOUT");
        }
        let flag = PricingRevampFlag::from_env();
        assert_eq!(flag.rollout_percentage, 0);
        assert!(!flag.is_enabled_for(uuid::Uuid::new_v4()));
    }
}
