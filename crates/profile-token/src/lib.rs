//! Profile-session tickets.
//!
//! The profile server is the only issuer and only verifier: a ticket scopes
//! this one service's profile calls, and is never forwarded to a game server
//! (those key players by display name). A ticket is
//! `v1.<base64url(payload)>.<base64url(hmac)>` over the payload text with
//! HMAC-SHA256; verification is constant-time and checks the expiry.
//!
//! Tickets are bearer credentials: treat the string as the session token. They
//! carry the account id and the display claims, never the profile itself.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Bumped only if the payload shape changes incompatibly.
pub const VERSION: &str = "v1";

/// Default ticket lifetime: a week. The client re-authenticates silently
/// against the profile server, so this only bounds a stolen ticket.
pub const DEFAULT_TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Maximum accepted ticket length, so a garbage header is cheap to reject.
pub const MAX_TOKEN_LEN: usize = 4096;

/// What a ticket asserts. `sub` is the stable account id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// Issue time (Unix seconds).
    pub iat: u64,
    /// Expiry (Unix seconds).
    pub exp: u64,
    /// Account id. Stable across renames.
    pub sub: String,
    /// Display name (already cleaned by the issuer).
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub persona: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Malformed,
    Expired,
    BadSignature,
    Encoding,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::Malformed => "malformed ticket",
            Error::Expired => "ticket expired",
            Error::BadSignature => "bad ticket signature",
            Error::Encoding => "bad ticket encoding",
        })
    }
}

impl std::error::Error for Error {}

/// Current Unix seconds.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Mint a ticket for `claims` over `secret`. Refuses an empty secret.
pub fn mint(secret: &[u8], claims: &Claims) -> Result<String, Error> {
    if secret.is_empty() {
        return Err(Error::Malformed);
    }
    let payload = serde_json::to_vec(claims).map_err(|_| Error::Encoding)?;
    let body = URL_SAFE_NO_PAD.encode(&payload);
    let signing_input = format!("{VERSION}.{body}");
    let sig = sign(secret, signing_input.as_bytes());
    Ok(format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(sig)))
}

/// Mint with the default lifetime and `now()` as the issue time.
pub fn mint_now(secret: &[u8], sub: &str, name: &str, persona: &str, cn_id: &str) -> Result<String, Error> {
    let iat = now();
    mint(
        secret,
        &Claims {
            iat,
            exp: iat + DEFAULT_TTL_SECS,
            sub: sub.to_string(),
            name: name.to_string(),
            persona: persona.to_string(),
            cn_id: cn_id.to_string(),
        },
    )
}

/// Verify `token` over `secret` and return its claims. Rejects a ticket that
/// expires at or before `now`, and one issued implausibly far in the future
/// (skew allowance of 300 s).
pub fn verify(secret: &[u8], token: &str) -> Result<Claims, Error> {
    if secret.is_empty() || token.len() > MAX_TOKEN_LEN {
        return Err(Error::Malformed);
    }
    let mut parts = token.split('.');
    let (Some(version), Some(body), Some(sig), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Error::Malformed);
    };
    if version != VERSION || body.is_empty() || sig.is_empty() {
        return Err(Error::Malformed);
    }
    let given = URL_SAFE_NO_PAD.decode(sig).map_err(|_| Error::Encoding)?;
    let signing_input = format!("{VERSION}.{body}");
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| Error::Malformed)?;
    mac.update(signing_input.as_bytes());
    mac.verify_slice(&given).map_err(|_| Error::BadSignature)?;
    let payload = URL_SAFE_NO_PAD.decode(body).map_err(|_| Error::Encoding)?;
    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| Error::Malformed)?;
    let t = now();
    if claims.exp <= t {
        return Err(Error::Expired);
    }
    if claims.iat > t + 300 {
        return Err(Error::Malformed);
    }
    if claims.sub.is_empty() {
        return Err(Error::Malformed);
    }
    Ok(claims)
}

fn sign(secret: &[u8], input: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(input);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-secret";

    fn claims() -> Claims {
        Claims {
            iat: now(),
            exp: now() + 60,
            sub: "a1".into(),
            name: "灯".into(),
            persona: "高松灯".into(),
            cn_id: "char_1".into(),
        }
    }

    #[test]
    fn round_trip() {
        let t = mint(SECRET, &claims()).unwrap();
        let got = verify(SECRET, &t).unwrap();
        assert_eq!(got.sub, "a1");
        assert_eq!(got.name, "灯");
    }

    #[test]
    fn rejects_tamper_and_other_secret() {
        let t = mint(SECRET, &claims()).unwrap();
        // Flip one bit in the decoded signature, keeping its length.
        let (head, sig) = t.rsplit_once('.').unwrap();
        let mut raw = URL_SAFE_NO_PAD.decode(sig).unwrap();
        raw[0] ^= 1;
        let bad = format!("{head}.{}", URL_SAFE_NO_PAD.encode(raw));
        assert_eq!(verify(SECRET, &bad), Err(Error::BadSignature));
        assert_eq!(verify(b"other", &t), Err(Error::BadSignature));
    }

    #[test]
    fn rejects_expired() {
        let mut c = claims();
        c.exp = now().saturating_sub(1);
        let t = mint(SECRET, &c).unwrap();
        assert_eq!(verify(SECRET, &t), Err(Error::Expired));
    }

    #[test]
    fn rejects_shape() {
        assert_eq!(verify(SECRET, ""), Err(Error::Malformed));
        assert_eq!(verify(SECRET, "v1.abc"), Err(Error::Malformed));
        assert_eq!(verify(b"", "v1.a.b"), Err(Error::Malformed));
    }
}
