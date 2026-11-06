/// Privacy enforcement layer.
///
/// Ensures private session is always on, no logging occurs, and telemetry/analytics
/// are stripped from outgoing requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Privacy {
    pub private_session: bool,
    pub no_logging: bool,
    pub limit_telemetry: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self {
            private_session: true,
            no_logging: true,
            limit_telemetry: true,
        }
    }
}

impl Privacy {
    pub fn from_config(private_session: bool, no_logging: bool, limit_telemetry: bool) -> Self {
        Self {
            private_session,
            no_logging,
            limit_telemetry,
        }
    }

    /// Always enforce private_session = true regardless of config (hard privacy guarantee).
    pub fn enforce_private_session(&self) -> bool {
        true
    }

    /// Whether logging is suppressed.
    pub fn is_logging_disabled(&self) -> bool {
        self.no_logging
    }

    /// Whether telemetry headers should be stripped.
    pub fn is_telemetry_limited(&self) -> bool {
        self.limit_telemetry
    }
}
