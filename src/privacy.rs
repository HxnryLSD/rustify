use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};

/// Privacy enforcement layer.
///
/// Owns the rules that keep Rustify from leaking data to Spotify or third
/// parties, and is consulted by every outgoing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Privacy {
    /// Report the user's private-session state to the user.
    ///
    /// Private session is an account-level setting that Spotify's own servers
    /// enforce; the public Web API has no parameter for it and ignores one
    /// that is sent. Rustify therefore cannot turn it on, and cannot read it
    /// for the account — only the active device reports it back.
    pub private_session: bool,
    /// Suppress local logging entirely.
    pub no_logging: bool,
    /// Strip analytics and telemetry headers.
    pub limit_telemetry: bool,
    /// Do not persist listening history locally.
    pub no_local_history: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self {
            private_session: true,
            no_logging: true,
            limit_telemetry: true,
            no_local_history: true,
        }
    }
}

impl Privacy {
    /// Build the policy from the persisted configuration.
    pub fn from_config(
        private_session: bool,
        no_logging: bool,
        limit_telemetry: bool,
        no_local_history: bool,
    ) -> Self {
        Self {
            private_session,
            no_logging,
            limit_telemetry,
            no_local_history,
        }
    }

    /// Whether the user asked to be told about private-session state.
    ///
    /// This does not enable private session — see [`Privacy::private_session`].
    pub fn wants_private_session(&self) -> bool {
        self.private_session
    }

    /// Whether logging is suppressed.
    pub fn is_logging_disabled(&self) -> bool {
        self.no_logging
    }

    /// Whether telemetry headers should be stripped.
    pub fn is_telemetry_limited(&self) -> bool {
        self.limit_telemetry
    }

    /// Whether local listening history may be written.
    pub fn is_local_history_disabled(&self) -> bool {
        self.no_local_history
    }

    /// Build the header set for an outgoing Spotify request.
    ///
    /// Only the headers that are actually needed are attached. No
    /// `X-Spotify-Client-Info`, no `X-Client-Id`, no analytics payload.
    pub fn request_headers(&self, access_token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {access_token}")) {
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers
    }
}

/// Header names that must never leave the process.
///
/// Kept as data so the list is auditable and testable.
pub const FORBIDDEN_HEADERS: &[&str] = &[
    "x-spotify-client-info",
    "x-client-id",
    "x-spotify-app-version",
    "x-spotify-platform",
    "x-spotify-device",
    "x-correlation-id",
    "x-spclient-uuid",
];

/// Remove any header that could carry tracking information.
pub fn strip_telemetry(headers: &mut HeaderMap) {
    for name in FORBIDDEN_HEADERS {
        headers.remove(*name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_carry_only_the_bearer_token() {
        let privacy = Privacy::default();
        let headers = privacy.request_headers("tok");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers.get("authorization").unwrap(), "Bearer tok");
        assert_eq!(headers.get("accept").unwrap(), "application/json");
        for name in FORBIDDEN_HEADERS {
            assert!(headers.get(*name).is_none(), "{name} must not be sent");
        }
    }

    #[test]
    fn telemetry_headers_are_stripped() {
        let mut headers = HeaderMap::new();
        headers.insert("x-spotify-client-info", HeaderValue::from_static("x"));
        headers.insert("x-client-id", HeaderValue::from_static("x"));
        headers.insert("accept", HeaderValue::from_static("application/json"));

        strip_telemetry(&mut headers);
        assert_eq!(headers.len(), 1);
        assert!(headers.get("accept").is_some());
    }

    #[test]
    fn private_session_is_only_a_display_preference() {
        // The Web API ignores the parameter, so the policy must not send one.
        assert!(Privacy::default().wants_private_session());
        let off = Privacy {
            private_session: false,
            ..Default::default()
        };
        assert!(!off.wants_private_session());
    }
}
