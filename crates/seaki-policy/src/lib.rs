pub const CAPABILITY_GRANT_VISIBILITY: &str = "opaque-id-only";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny,
}

impl PolicyDecision {
    pub const fn permits_side_effect(self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_default_shape_keeps_grants_opaque() {
        assert_eq!(CAPABILITY_GRANT_VISIBILITY, "opaque-id-only");
        assert!(PolicyDecision::Allow.permits_side_effect());
        assert!(!PolicyDecision::Deny.permits_side_effect());
    }
}
