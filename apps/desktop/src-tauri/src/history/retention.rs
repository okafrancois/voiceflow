use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    Never,
    #[serde(rename = "days_7")]
    Days7,
    #[serde(rename = "days_30")]
    Days30,
    #[serde(rename = "days_90")]
    Days90,
    Forever,
}

impl RetentionPolicy {
    pub fn max_age_days(self) -> Option<u64> {
        match self {
            Self::Never => Some(0),
            Self::Days7 => Some(7),
            Self::Days30 => Some(30),
            Self::Days90 => Some(90),
            Self::Forever => None,
        }
    }

    pub fn retains_new_data(self) -> bool {
        self != Self::Never
    }
}

#[cfg(test)]
mod tests {
    use super::RetentionPolicy;

    #[test]
    fn policies_use_stable_snake_case_values() {
        assert_eq!(
            serde_json::to_string(&RetentionPolicy::Days30).unwrap(),
            "\"days_30\""
        );
        assert_eq!(
            serde_json::from_str::<RetentionPolicy>("\"forever\"").unwrap(),
            RetentionPolicy::Forever
        );
    }

    #[test]
    fn never_is_the_only_policy_that_rejects_new_data() {
        assert!(!RetentionPolicy::Never.retains_new_data());
        assert!(RetentionPolicy::Days7.retains_new_data());
        assert!(RetentionPolicy::Forever.retains_new_data());
    }
}
