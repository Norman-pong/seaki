pub const DAEMON_ENTRYPOINT: &str = "local-daemon-ingress";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonIngressContract {
    pub accepts_inert_events: bool,
    pub exposes_frontend_api: bool,
}

impl DaemonIngressContract {
    pub const fn m0() -> Self {
        Self {
            accepts_inert_events: true,
            exposes_frontend_api: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_m0_contract_is_inert_ingress() {
        let contract = DaemonIngressContract::m0();

        assert_eq!(DAEMON_ENTRYPOINT, "local-daemon-ingress");
        assert!(contract.accepts_inert_events);
        assert!(contract.exposes_frontend_api);
    }
}
