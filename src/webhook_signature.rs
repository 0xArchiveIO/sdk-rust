//! Verify the signature on an inbound 0xArchive webhook delivery.
//!
//! Every delivery 0xArchive sends carries four headers:
//!
//! | Header | Value |
//! |---|---|
//! | `content-type` | `application/json` |
//! | `0xa-signature` | `t=<unix seconds>,v1=<hex>[,v1=<hex>]` |
//! | `0xa-event-id` | the event UUID, stable across retries and redelivery |
//! | `0xa-event-type` | the event type, for example `webhook.test` |
//!
//! plus a `user-agent` of `0xArchive-Webhooks/1.0`. Header names are emitted
//! lowercase, but HTTP header names are case insensitive, so look them up
//! case insensitively rather than relying on the casing on the wire.
//!
//! There is no separate timestamp header. The timestamp lives inside
//! `0xa-signature` as `t`, and because it is the first component of the signed
//! string it cannot be moved without breaking the signature.
//!
//! # The signed bytes
//!
//! ```text
//! signed_payload = <t> || "." || <raw request body>
//! signature      = lowercase_hex(HMAC_SHA256(key = whole secret string, signed_payload))
//! ```
//!
//! One ASCII full stop, no newline, no length prefix, no trailing whitespace.
//! The event id, the event type, the destination URL and the HTTP method are
//! **not** signed.
//!
//! # The one mistake that breaks everything
//!
//! Sign the bytes you received. Do not re-serialise the JSON first.
//!
//! The body is rendered by PostgreSQL's `jsonb` output, so its key order and
//! spacing match neither the emitter's struct order nor any JSON library's
//! default output. `serde_json::to_string(&value)`, `JSON.stringify(body)` and
//! `json.dumps(payload)` all produce different bytes and all fail. Capture the
//! raw body before a parser touches it, and pass those bytes here.
//!
//! # Usage
//!
//! ```
//! use oxarchive::webhook_signature::{SignatureError, WebhookVerifier};
//!
//! # fn main() -> Result<(), SignatureError> {
//! let verifier = WebhookVerifier::new(
//!     "whsec_0000000000000000000000000000000000000000000000000000000000000000",
//! );
//!
//! let raw_body: &[u8] = br#"{"id": "11111111-1111-4111-8111-111111111111", "data": {"message": "Test event from 0xArchive."}, "type": "webhook.test", "observed_at": "2026-09-19T00:00:00+00:00", "schema_version": 1}"#;
//! let header = "t=1758240000,v1=027f40e95c9aa4e8097c22493f6f019ad25407ddf35b13103f95e5501d49ec0b";
//!
//! // `verify` also checks freshness against the system clock; this example
//! // pins the clock so the vector keeps verifying forever.
//! verifier.verify_at(raw_body, header, 1_758_240_000)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Rotating the secret
//!
//! `POST /v1/webhooks/endpoints/{id}/rotate` installs a new secret and keeps
//! the previous one valid for 24 hours. During that window every delivery
//! carries two `v1` values over the same signed payload, one per secret, and
//! the header does not say which is which. Hold both secrets until you have
//! finished rolling:
//!
//! ```
//! # use oxarchive::webhook_signature::WebhookVerifier;
//! let verifier = WebhookVerifier::with_secrets([
//!     "whsec_new...",  // returned by rotate
//!     "whsec_old...",  // valid for 24 more hours
//! ]);
//! ```
//!
//! Only one previous secret is ever carried. Rotating twice inside the window
//! overwrites it, and the original stops verifying immediately.
//!
//! # After a delivery verifies
//!
//! Deduplicate on `0xa-event-id`. Delivery is at least once, and a manual
//! redelivery deliberately reuses the event id. Do not deduplicate on the
//! signature or on `t`: both change on every attempt.
//!
//! Answer 2xx within 10 seconds and do the work out of band. Anything outside
//! 200 to 299, redirects included, counts as a failure and enters the retry
//! ladder (5s, 30s, 2m, 10m, 1h, then hourly, capped at 24 hours). Ten
//! consecutive failures spanning at least six hours auto disable the endpoint,
//! which `POST /v1/webhooks/endpoints/{id}/enable` clears.
//!
//! If a delivery fails to verify, answer 4xx and log it. A 5xx just replays
//! the same bad delivery at you for a day.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Header carrying the signature envelope.
pub const SIGNATURE_HEADER: &str = "0xa-signature";
/// Header carrying the event UUID. Deduplicate on this.
pub const EVENT_ID_HEADER: &str = "0xa-event-id";
/// Header carrying the event type, for example `webhook.test`.
pub const EVENT_TYPE_HEADER: &str = "0xa-event-type";
/// `user-agent` sent by the dispatcher.
pub const USER_AGENT: &str = "0xArchive-Webhooks/1.0";

/// Default replay tolerance, in seconds.
///
/// `t` is generated per attempt rather than per event, so a retry an hour
/// later still arrives with a fresh timestamp. Nothing legitimate produces a
/// stale `t` beyond ordinary network and clock skew, which is why five
/// minutes is comfortable rather than tight.
pub const DEFAULT_TOLERANCE_SECS: i64 = 300;

/// Prefix every 0xArchive signing secret carries. It is part of the HMAC key:
/// do not strip it, and do not hex decode what follows it.
pub const SECRET_PREFIX: &str = "whsec_";

/// Why a delivery was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    /// The `0xa-signature` header was absent or empty.
    #[error("missing 0xa-signature header")]
    MissingHeader,

    /// The header was present but carried no usable `t` and `v1` pair.
    #[error("malformed 0xa-signature header: {0}")]
    MalformedHeader(&'static str),

    /// The delivery is outside the replay window.
    #[error("timestamp is {skew_secs}s from now, outside the {tolerance_secs}s tolerance")]
    Stale { skew_secs: i64, tolerance_secs: i64 },

    /// The verifier was built without any secrets.
    #[error("no signing secrets configured")]
    NoSecrets,

    /// Every signature in the header failed against every secret held.
    #[error("signature does not match")]
    Mismatch,
}

/// The parsed contents of a `0xa-signature` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSignature {
    /// `t` exactly as it appeared in the header. This literal substring is
    /// what goes back into the signed string; reformatting it changes the
    /// bytes and breaks the MAC.
    pub timestamp_raw: String,
    /// `t` parsed to Unix seconds, for the freshness check only.
    pub timestamp: i64,
    /// Every `v1` value in the header, lowercased, in the order sent. One
    /// normally, two during a rotation overlap.
    pub signatures: Vec<String>,
}

/// Parse a `0xa-signature` header value.
///
/// Splits on commas and collects **every** `v1`. Reading only the first one is
/// a real bug: it passes in steady state and fails intermittently the moment
/// somebody rotates a secret.
pub fn parse_signature_header(header: &str) -> Result<ParsedSignature, SignatureError> {
    if header.trim().is_empty() {
        return Err(SignatureError::MissingHeader);
    }

    let mut timestamp_raw: Option<&str> = None;
    let mut signatures = Vec::new();

    for element in header.split(',') {
        let element = element.trim();
        // Split on the FIRST '=' only: a value could contain one.
        let Some((key, value)) = element.split_once('=') else {
            continue;
        };
        match key.trim() {
            "t" if timestamp_raw.is_none() => timestamp_raw = Some(value.trim()),
            "v1" => signatures.push(value.trim().to_ascii_lowercase()),
            _ => {}
        }
    }

    let timestamp_raw =
        timestamp_raw.ok_or(SignatureError::MalformedHeader("no t= component"))?;
    if signatures.is_empty() {
        return Err(SignatureError::MalformedHeader("no v1= component"));
    }
    let timestamp: i64 = timestamp_raw
        .parse()
        .map_err(|_| SignatureError::MalformedHeader("t is not an integer"))?;

    Ok(ParsedSignature {
        timestamp_raw: timestamp_raw.to_string(),
        timestamp,
        signatures,
    })
}

/// Verifies 0xArchive webhook deliveries against one or more signing secrets.
///
/// Holding more than one secret is the normal state while rotating; see the
/// module docs.
#[derive(Clone)]
pub struct WebhookVerifier {
    secrets: Vec<String>,
    tolerance_secs: i64,
}

/// Prints how many secrets are held, never the secrets themselves.
///
/// A verifier is long lived and usually ends up inside something else that
/// derives `Debug`: application state, an error report, a tracing span. The
/// derived implementation would put every `whsec_...` string in that log line,
/// and a signing secret in a log is a signing secret that has left your
/// control. This is the one type in the SDK that holds a secret for the life
/// of the process, so it is the one that redacts.
impl std::fmt::Debug for WebhookVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookVerifier")
            .field("secrets", &format_args!("{} held, redacted", self.secrets.len()))
            .field("tolerance_secs", &self.tolerance_secs)
            .finish()
    }
}

impl WebhookVerifier {
    /// Build a verifier for a single secret, the whole `whsec_...` string as
    /// the API returned it.
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secrets: vec![secret.into()],
            tolerance_secs: DEFAULT_TOLERANCE_SECS,
        }
    }

    /// Build a verifier holding several secrets, for example the new and the
    /// previous one during a rotation overlap. A delivery is accepted when any
    /// signature in the header matches any secret held.
    pub fn with_secrets<I, S>(secrets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            secrets: secrets.into_iter().map(Into::into).collect(),
            tolerance_secs: DEFAULT_TOLERANCE_SECS,
        }
    }

    /// Add another secret to try.
    pub fn add_secret(mut self, secret: impl Into<String>) -> Self {
        self.secrets.push(secret.into());
        self
    }

    /// Override the replay tolerance. Defaults to
    /// [`DEFAULT_TOLERANCE_SECS`]. Widening it past a few minutes buys nothing
    /// and lengthens the window an observed delivery can be replayed in.
    pub fn tolerance_secs(mut self, secs: i64) -> Self {
        self.tolerance_secs = secs;
        self
    }

    /// The configured replay tolerance, in seconds.
    pub fn tolerance(&self) -> i64 {
        self.tolerance_secs
    }

    /// Verify a delivery against the current system clock.
    ///
    /// `raw_body` must be the bytes as received, before any JSON parsing.
    /// `signature_header` is the `0xa-signature` value.
    pub fn verify(
        &self,
        raw_body: &[u8],
        signature_header: &str,
    ) -> Result<ParsedSignature, SignatureError> {
        self.verify_at(raw_body, signature_header, unix_now())
    }

    /// Verify a delivery against a caller supplied clock, in Unix seconds.
    ///
    /// Same checks as [`verify`](Self::verify); useful in tests and anywhere
    /// the clock is injected.
    pub fn verify_at(
        &self,
        raw_body: &[u8],
        signature_header: &str,
        now_unix_secs: i64,
    ) -> Result<ParsedSignature, SignatureError> {
        if self.secrets.is_empty() {
            return Err(SignatureError::NoSecrets);
        }

        let parsed = parse_signature_header(signature_header)?;

        // Freshness first: it is the cheap check, and a signature with no
        // freshness check is replayable forever by anyone who once observed a
        // delivery. The URL is not part of the signed string, so an observed
        // delivery can also be replayed at a different path on the same host.
        let skew = now_unix_secs.saturating_sub(parsed.timestamp).saturating_abs();
        if skew > self.tolerance_secs {
            return Err(SignatureError::Stale {
                skew_secs: skew,
                tolerance_secs: self.tolerance_secs,
            });
        }

        // Decode once. A candidate that is not 32 bytes of hex cannot match
        // anything, so it is dropped rather than compared.
        let candidates: Vec<[u8; 32]> = parsed
            .signatures
            .iter()
            .filter_map(|s| decode_hex32(s))
            .collect();
        if candidates.is_empty() {
            return Err(SignatureError::MalformedHeader(
                "no v1= value is 64 hex characters",
            ));
        }

        for secret in &self.secrets {
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
                .expect("HMAC accepts a key of any length");
            // The literal `t` substring, one ASCII full stop, then the body
            // bytes untouched. No trailing newline, no re-encoding.
            mac.update(parsed.timestamp_raw.as_bytes());
            mac.update(b".");
            mac.update(raw_body);

            for candidate in &candidates {
                // `verify_slice` is the constant time comparison; it also
                // length checks, so a short candidate fails closed.
                if mac.clone().verify_slice(candidate).is_ok() {
                    return Ok(parsed);
                }
            }
        }

        Err(SignatureError::Mismatch)
    }
}

/// Seconds since the Unix epoch. Saturates at 0 if the clock is before 1970.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Decode exactly 64 hex characters, either case, into 32 bytes.
fn decode_hex32(s: &str) -> Option<[u8; 32]> {
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        out[i] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Conformance vectors. The secrets are documentation placeholders, and
    /// the signatures were produced by the same scheme the dispatcher uses.
    const T: i64 = 1_758_240_000;
    const BODY: &[u8] = br#"{"id": "11111111-1111-4111-8111-111111111111", "data": {"message": "Test event from 0xArchive."}, "type": "webhook.test", "observed_at": "2026-09-19T00:00:00+00:00", "schema_version": 1}"#;

    const CURRENT: &str =
        "whsec_0000000000000000000000000000000000000000000000000000000000000000";
    const PREVIOUS: &str =
        "whsec_1111111111111111111111111111111111111111111111111111111111111111";

    /// Vector A: steady state, one signature, signed with CURRENT.
    const VECTOR_A: &str =
        "t=1758240000,v1=027f40e95c9aa4e8097c22493f6f019ad25407ddf35b13103f95e5501d49ec0b";

    /// Vector B: rotation overlap. Same payload, CURRENT first, then PREVIOUS.
    const VECTOR_B: &str = "t=1758240000,\
         v1=027f40e95c9aa4e8097c22493f6f019ad25407ddf35b13103f95e5501d49ec0b,\
         v1=f8e6ae6781adad70ed0f94773fa2745147b718136e83fc22cafa09ded928095c";

    #[test]
    fn the_vectors_describe_the_bytes_they_claim_to() {
        assert_eq!(BODY.len(), 186);
        assert_eq!(format!("{T}.").len() + BODY.len(), 197);
    }

    #[test]
    fn vector_a_verifies_with_the_current_secret() {
        let v = WebhookVerifier::new(CURRENT);
        let parsed = v.verify_at(BODY, VECTOR_A, T).unwrap();
        assert_eq!(parsed.timestamp, T);
        assert_eq!(parsed.timestamp_raw, "1758240000");
        assert_eq!(parsed.signatures.len(), 1);
    }

    #[test]
    fn a_holder_of_only_the_previous_secret_rejects_vector_a() {
        let v = WebhookVerifier::new(PREVIOUS);
        assert_eq!(
            v.verify_at(BODY, VECTOR_A, T).unwrap_err(),
            SignatureError::Mismatch
        );
    }

    /// The rotation test that catches `header.split("v1=")[1]`. A verifier
    /// that reads only the first signature passes vector A and fails here.
    #[test]
    fn during_rotation_either_secret_alone_accepts_vector_b() {
        for secret in [CURRENT, PREVIOUS] {
            let v = WebhookVerifier::new(secret);
            assert!(
                v.verify_at(BODY, VECTOR_B, T).is_ok(),
                "holder of {} must accept the overlap header",
                &secret[..12]
            );
        }
    }

    #[test]
    fn holding_both_secrets_accepts_both_vectors() {
        let v = WebhookVerifier::with_secrets([CURRENT, PREVIOUS]);
        assert!(v.verify_at(BODY, VECTOR_A, T).is_ok());
        assert!(v.verify_at(BODY, VECTOR_B, T).is_ok());
    }

    #[test]
    fn add_secret_composes_the_same_way() {
        let v = WebhookVerifier::new(CURRENT).add_secret(PREVIOUS);
        assert!(v.verify_at(BODY, VECTOR_B, T).is_ok());
    }

    #[test]
    fn one_changed_byte_in_the_body_is_refused() {
        let mut tampered = BODY.to_vec();
        let last = tampered.len() - 2;
        tampered[last] = b'2';
        let v = WebhookVerifier::new(CURRENT);
        assert_eq!(
            v.verify_at(&tampered, VECTOR_A, T).unwrap_err(),
            SignatureError::Mismatch
        );
    }

    #[test]
    fn a_trailing_newline_on_the_body_is_refused() {
        // Proves nothing in the verifier trims or normalises the body.
        let mut padded = BODY.to_vec();
        padded.push(b'\n');
        let v = WebhookVerifier::new(CURRENT);
        assert_eq!(
            v.verify_at(&padded, VECTOR_A, T).unwrap_err(),
            SignatureError::Mismatch
        );
    }

    #[test]
    fn re_serialising_the_json_produces_different_bytes_and_is_refused() {
        // The single most common way to ship a broken verifier.
        let value: serde_json::Value = serde_json::from_slice(BODY).unwrap();
        let reserialised = serde_json::to_vec(&value).unwrap();
        assert_ne!(reserialised, BODY.to_vec());
        let v = WebhookVerifier::new(CURRENT);
        assert_eq!(
            v.verify_at(&reserialised, VECTOR_A, T).unwrap_err(),
            SignatureError::Mismatch
        );
    }

    #[test]
    fn a_stale_timestamp_is_refused_at_the_default_tolerance() {
        let v = WebhookVerifier::new(CURRENT);
        let err = v.verify_at(BODY, VECTOR_A, T + 600).unwrap_err();
        assert_eq!(
            err,
            SignatureError::Stale {
                skew_secs: 600,
                tolerance_secs: DEFAULT_TOLERANCE_SECS,
            }
        );
        // A receiver clock running behind the server is just as far out.
        assert!(matches!(
            v.verify_at(BODY, VECTOR_A, T - 600).unwrap_err(),
            SignatureError::Stale { .. }
        ));
        // Inside the window, either direction, it verifies.
        assert!(v.verify_at(BODY, VECTOR_A, T + 299).is_ok());
        assert!(v.verify_at(BODY, VECTOR_A, T - 299).is_ok());
        // Exactly at the tolerance is still accepted; past it is not.
        assert!(v.verify_at(BODY, VECTOR_A, T + 300).is_ok());
        assert!(v.verify_at(BODY, VECTOR_A, T + 301).is_err());
    }

    #[test]
    fn the_tolerance_is_configurable() {
        let v = WebhookVerifier::new(CURRENT).tolerance_secs(3600);
        assert_eq!(v.tolerance(), 3600);
        assert!(v.verify_at(BODY, VECTOR_A, T + 600).is_ok());
    }

    #[test]
    fn uppercase_hex_still_verifies() {
        let upper = VECTOR_A.to_uppercase().replace("T=", "t=").replace("V1=", "v1=");
        let v = WebhookVerifier::new(CURRENT);
        assert!(v.verify_at(BODY, &upper, T).is_ok());
    }

    #[test]
    fn the_secret_prefix_is_part_of_the_key() {
        // Stripping `whsec_` or hex decoding the remainder are both wrong.
        let stripped = CURRENT.trim_start_matches(SECRET_PREFIX);
        let v = WebhookVerifier::new(stripped);
        assert_eq!(
            v.verify_at(BODY, VECTOR_A, T).unwrap_err(),
            SignatureError::Mismatch
        );
    }

    #[test]
    fn malformed_and_missing_headers_are_named() {
        let v = WebhookVerifier::new(CURRENT);
        assert_eq!(
            v.verify_at(BODY, "", T).unwrap_err(),
            SignatureError::MissingHeader
        );
        assert_eq!(
            v.verify_at(BODY, "   ", T).unwrap_err(),
            SignatureError::MissingHeader
        );
        assert!(matches!(
            v.verify_at(BODY, "v1=deadbeef", T).unwrap_err(),
            SignatureError::MalformedHeader(_)
        ));
        assert!(matches!(
            v.verify_at(BODY, "t=1758240000", T).unwrap_err(),
            SignatureError::MalformedHeader(_)
        ));
        assert!(matches!(
            v.verify_at(BODY, "t=not-a-number,v1=ab", T).unwrap_err(),
            SignatureError::MalformedHeader(_)
        ));
        // Right shape, wrong length: nothing to compare against.
        assert!(matches!(
            v.verify_at(BODY, "t=1758240000,v1=abcd", T).unwrap_err(),
            SignatureError::MalformedHeader(_)
        ));
    }

    #[test]
    fn an_absurd_timestamp_is_refused_rather_than_overflowing() {
        // A hostile `t` must not be able to panic a receiver on the abs().
        let v = WebhookVerifier::new(CURRENT);
        let header = format!("t={},v1={}", i64::MIN, "0".repeat(64));
        assert!(matches!(
            v.verify_at(BODY, &header, T).unwrap_err(),
            SignatureError::Stale { .. }
        ));
        let header = format!("t={},v1={}", i64::MAX, "0".repeat(64));
        assert!(matches!(
            v.verify_at(BODY, &header, T).unwrap_err(),
            SignatureError::Stale { .. }
        ));
        // Out of i64 range at all is a malformed header, not a panic.
        assert!(matches!(
            v.verify_at(BODY, "t=99999999999999999999,v1=ab", T).unwrap_err(),
            SignatureError::MalformedHeader(_)
        ));
    }

    #[test]
    fn a_verifier_with_no_secrets_fails_closed() {
        let v = WebhookVerifier::with_secrets(Vec::<String>::new());
        assert_eq!(
            v.verify_at(BODY, VECTOR_A, T).unwrap_err(),
            SignatureError::NoSecrets
        );
    }

    #[test]
    fn the_header_parser_keeps_the_literal_timestamp_and_every_signature() {
        let parsed = parse_signature_header(VECTOR_B).unwrap();
        assert_eq!(parsed.timestamp_raw, "1758240000");
        assert_eq!(parsed.timestamp, 1_758_240_000);
        assert_eq!(parsed.signatures.len(), 2);
        assert!(parsed.signatures.iter().all(|s| s.len() == 64));
        // First `t` wins, and unknown components are ignored rather than
        // treated as signatures.
        let parsed =
            parse_signature_header("scheme=v1,t=1758240000,t=999,v1=aa,extra=x").unwrap();
        assert_eq!(parsed.timestamp_raw, "1758240000");
        assert_eq!(parsed.signatures, vec!["aa".to_string()]);
    }

    /// A vector derived independently of the pair above: different secret,
    /// different body, computed outside this crate straight from the scheme
    /// the dispatcher documents (`hex(HMAC_SHA256(whole secret, "<t>." + raw
    /// body))`). Vectors written alongside an implementation can agree with
    /// that implementation's own mistake; one computed elsewhere cannot.
    ///
    /// The body carries a multibyte character and a JSON escape, so any
    /// re-encoding on the way in shows up as a mismatch rather than passing.
    #[test]
    fn an_independently_computed_vector_verifies_and_a_tampered_body_does_not() {
        const SECRET: &str =
            "whsec_2222222222222222222222222222222222222222222222222222222222222222";
        const TS: i64 = 1_758_285_296;
        const HEADER: &str =
            "t=1758285296,v1=73856d4c11e53523dd21f79c6ca0743290a63f09589fbaf32418d3f2b2ff72f8";
        let body = r#"{"id": "22222222-2222-4222-8222-222222222222", "data": {"note": "sizing \u2192 café", "notional_usd": 250000.0}, "type": "market.liquidation", "observed_at": "2026-09-19T12:34:56+00:00", "schema_version": 1}"#;
        assert_eq!(body.len(), 208, "the vector is over these exact bytes");

        let v = WebhookVerifier::new(SECRET);
        v.verify_at(body.as_bytes(), HEADER, TS)
            .expect("the scheme the dispatcher signs with must verify here");

        // One digit of the notional changed: same length, same shape, refused.
        let tampered = body.replace("250000.0", "250001.0");
        assert_eq!(tampered.len(), body.len());
        assert_eq!(
            v.verify_at(tampered.as_bytes(), HEADER, TS).unwrap_err(),
            SignatureError::Mismatch
        );

        // And a holder of a different secret cannot accept it.
        assert_eq!(
            WebhookVerifier::new(CURRENT)
                .verify_at(body.as_bytes(), HEADER, TS)
                .unwrap_err(),
            SignatureError::Mismatch
        );
    }

    #[test]
    fn debug_never_prints_a_secret() {
        // A verifier tends to live inside something that derives Debug.
        let rendered = format!("{:?}", WebhookVerifier::with_secrets([CURRENT, PREVIOUS]));
        assert!(!rendered.contains("whsec_"), "{rendered}");
        assert!(!rendered.contains(&CURRENT[6..]), "{rendered}");
        assert!(rendered.contains("2 held, redacted"), "{rendered}");
        assert!(rendered.contains("tolerance_secs: 300"), "{rendered}");
    }

    #[test]
    fn hex_decoding_rejects_anything_that_is_not_64_hex_characters() {
        assert!(decode_hex32(&"a".repeat(64)).is_some());
        assert!(decode_hex32(&"A".repeat(64)).is_some());
        assert!(decode_hex32(&"a".repeat(63)).is_none());
        assert!(decode_hex32(&"a".repeat(65)).is_none());
        assert!(decode_hex32(&"g".repeat(64)).is_none());
        assert!(decode_hex32("").is_none());
    }
}
