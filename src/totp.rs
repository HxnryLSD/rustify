use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// Cipher bytes for the Spotify web-player TOTP secret, version 61.
///
/// The web player derives a time-based one-time password from a versioned
/// secret embedded in its JavaScript bundle and sends it with every token
/// request. The stored form is a simple XOR mask over the ASCII digits of
/// the base32 secret.
///
/// This is the fallback: the live value is read from the bundle at runtime so
/// a rotation does not break the sign-in.
pub const TOTP_SECRET_CIPHER_BYTES: [u8; 26] = [
    44, 55, 47, 42, 70, 40, 34, 114, 76, 74, 50, 111, 120, 97, 75, 76, 94, 102, 43, 69, 49, 120,
    118, 80, 64, 78,
];

/// Version identifier of the secret above.
pub const TOTP_VERSION: u32 = 61;

/// Length of the generated one-time password.
const OTP_DIGITS: u32 = 6;

/// Length of the TOTP time step, in seconds.
const OTP_INTERVAL: u64 = 30;

/// The web player's TOTP secret and the version it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPlayerSecret {
    /// The base32 secret used as the HMAC key.
    pub secret_base32: String,
    /// The version of the secret, sent as `totpVer`.
    pub version: u32,
}

impl WebPlayerSecret {
    /// The fallback secret, used when the live bundle cannot be read.
    pub fn fallback() -> Self {
        Self {
            secret_base32: recover_secret(&TOTP_SECRET_CIPHER_BYTES),
            version: TOTP_VERSION,
        }
    }

    /// Compute the one-time password for a point in time.
    pub fn totp(&self, unix_time: i64) -> String {
        generate_totp(&self.secret_base32, unix_time)
    }
}

/// Recover the base32 TOTP secret from the stored cipher bytes.
///
/// Each byte is unmasked with a position-dependent mask, the unmasked values
/// are rendered in decimal, and the resulting text is base32-encoded.
pub fn recover_secret(cipher_bytes: &[u8]) -> String {
    let digits: String = cipher_bytes
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let unmasked = value ^ ((index % 33) as u8 + 9);
            unmasked.to_string()
        })
        .collect();

    BASE32_NOPAD.encode(digits.as_bytes())
}

/// Compute a 6-digit TOTP code for the given unix timestamp.
pub fn generate_totp(secret_base32: &str, unix_time: i64) -> String {
    let Ok(key) = BASE32_NOPAD.decode(secret_base32.as_bytes()) else {
        return String::new();
    };
    let counter = (unix_time.max(0) as u64) / OTP_INTERVAL;

    let mut mac = match HmacSha1::new_from_slice(&key) {
        Ok(mac) => mac,
        Err(_) => return String::new(),
    };
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();

    // RFC 6238 dynamic truncation.
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);
    let modulus = 10u32.pow(OTP_DIGITS);
    format!("{:0width$}", binary % modulus, width = OTP_DIGITS as usize)
}

/// The base32 secret the web player uses, for verification.
pub fn web_player_secret() -> String {
    recover_secret(&TOTP_SECRET_CIPHER_BYTES)
}

/// The current unix time in seconds.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Find the web-player bundle URL in the player page HTML.
///
/// The page loads several bundles; the one carrying the secret is the main
/// `web-player.<hash>.js` and not a `vendor~` or `.css.` chunk.
pub fn discover_bundle_url(html: &str) -> Option<String> {
    const PREFIX: &str = "/cdn/build/web-player/";
    let mut names: Vec<String> = Vec::new();
    let mut rest = html;

    while let Some(position) = rest.find(PREFIX) {
        let after = &rest[position + PREFIX.len()..];
        let end = after.find('"').unwrap_or(after.len());
        let mut name = after[..end].to_string();
        if !name.ends_with(".js") {
            name.push_str(".js");
        }
        if !name.contains('?') {
            names.push(name);
        }
        rest = &after[end..];
    }

    names.sort();
    names.dedup();
    let chosen = names
        .iter()
        .find(|name| name.starts_with("web-player.") && !name.contains(".css."))
        .or_else(|| names.iter().find(|name| name.starts_with("web-player.")))
        .or_else(|| names.first())?;
    Some(format!(
        "https://open.spotifycdn.com/cdn/build/web-player/{chosen}"
    ))
}

/// Extract the TOTP secret from the web-player bundle.
///
/// The bundle stores the secrets as `{secret:'<ascii>',version:<n>}`. The
/// highest version wins, because the web player keeps the previous ones
/// around for a while.
pub fn extract_secret_from_bundle(js: &str) -> Option<WebPlayerSecret> {
    let mut best: Option<WebPlayerSecret> = None;

    for marker in ["secret:'", "secret:\"", "secret:"] {
        let mut cursor = 0usize;
        while let Some(position) = js[cursor..].find(marker) {
            let start = cursor + position + marker.len();
            let quote = js[start..].chars().next()?;
            if quote != '\'' && quote != '"' {
                cursor = start;
                continue;
            }

            let body_start = start + 1;
            let Some(end) = js[body_start..].find(quote) else {
                cursor = body_start;
                continue;
            };
            let ascii = &js[body_start..body_start + end];

            // The version sits right after the secret.
            let tail = &js[body_start + end..];
            let version = tail
                .find("version:")
                .and_then(|at| {
                    tail[at + "version:".len()..]
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<u32>()
                        .ok()
                })
                .filter(|version| *version > 0);

            if let Some(version) = version
                && !ascii.is_empty()
                && ascii.chars().all(|c| (' '..='~').contains(&c))
            {
                let secret = recover_secret(ascii.as_bytes());
                let candidate = WebPlayerSecret {
                    secret_base32: secret,
                    version,
                };
                if best
                    .as_ref()
                    .is_none_or(|current| candidate.version > current.version)
                {
                    best = Some(candidate);
                }
            }

            cursor = body_start + end;
        }

        if best.is_some() {
            break;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_is_stable_base32() {
        let secret = recover_secret(&TOTP_SECRET_CIPHER_BYTES);
        assert!(!secret.is_empty());
        assert!(
            secret
                .chars()
                .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c)),
            "unexpected characters in {secret}"
        );
        assert_eq!(secret, recover_secret(&TOTP_SECRET_CIPHER_BYTES));
    }

    #[test]
    fn recovered_secret_matches_the_web_player_secret() {
        assert_eq!(
            web_player_secret(),
            "GM3TMMJTGYZTQNZVGM4DINJZHA4TGOBYGMZTCMRTGEYDSMJRHE4TEOBUG4YTCMRUGQ4DQOJUGQYTAMRRGA2TCMJSHE3TCMBY"
        );
    }

    #[test]
    fn totp_is_six_digits() {
        let secret = recover_secret(&TOTP_SECRET_CIPHER_BYTES);
        let code = generate_totp(&secret, 1_700_000_000);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn totp_is_stable_within_a_time_step() {
        let secret = recover_secret(&TOTP_SECRET_CIPHER_BYTES);
        // 1_700_000_010 and 1_700_000_029 fall in the same 30 second step.
        assert_eq!(
            generate_totp(&secret, 1_700_000_010),
            generate_totp(&secret, 1_700_000_029)
        );
        assert_ne!(
            generate_totp(&secret, 1_700_000_010),
            generate_totp(&secret, 1_700_000_040)
        );
    }

    #[test]
    fn invalid_secret_yields_empty_code() {
        assert_eq!(generate_totp("not base32!!!", 1_700_000_000), "");
    }

    #[test]
    fn web_player_totp_is_six_digits() {
        let code = WebPlayerSecret::fallback().totp(now_unix());
        assert_eq!(code.len(), 6);
    }

    #[test]
    fn discovers_the_main_bundle() {
        let html = r#"<script src="/cdn/build/web-player/vendor~web-player.8f96401d.js"></script>
                      <script src="/cdn/build/web-player/web-player.7848a69e.css.js"></script>
                      <script src="/cdn/build/web-player/web-player.899a3ca1.js"></script>"#;
        assert_eq!(
            discover_bundle_url(html).as_deref(),
            Some("https://open.spotifycdn.com/cdn/build/web-player/web-player.899a3ca1.js")
        );
    }

    #[test]
    fn extracts_the_secret_from_a_bundle_snippet() {
        // The shape the web player uses: several versions, the newest wins.
        let js =
            "var a=[{secret:',7/*F(\"rLJ2oxaKL^f+E1xvP@N',version:61},{secret:'old',version:59}]";
        let secret = extract_secret_from_bundle(js).expect("the secret is found");
        assert_eq!(secret.version, 61);
        assert_eq!(secret.secret_base32, web_player_secret());
    }

    #[test]
    fn extraction_ignores_noise() {
        assert!(extract_secret_from_bundle("no secrets here at all").is_none());
        assert!(extract_secret_from_bundle("").is_none());
    }

    #[test]
    fn fallback_secret_matches_the_current_version() {
        let fallback = WebPlayerSecret::fallback();
        assert_eq!(fallback.version, TOTP_VERSION);
        assert_eq!(fallback.secret_base32, web_player_secret());
    }
}
