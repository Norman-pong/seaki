pub const SEARCH_RESULT_DISCLOSURE: &str = "candidate-ids-first";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexGeneration(pub u64);

impl IndexGeneration {
    pub const STALE: Self = Self(0);

    pub const fn is_stale(self) -> bool {
        self.0 == Self::STALE.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_m0_contract_starts_from_candidate_ids() {
        assert_eq!(SEARCH_RESULT_DISCLOSURE, "candidate-ids-first");
        assert!(IndexGeneration::STALE.is_stale());
        assert!(!IndexGeneration(1).is_stale());
    }
}
