//! Cloudflare Access JWT verification (shadow phase).
//!
//! Verifies the signed `Cf-Access-Jwt-Assertion` against a pinned JWKS fetched from
//! the team's `cdn-cgi/access/certs` endpoint. RS256. This is used in SHADOW mode
//! (CF_JWT_VERIFY=0): it only observes/logs — identity still comes from the plaintext
//! `Cf-Access-Authenticated-User-Email` header and nothing is blocked.

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

/// Successfully verified CF Access identity claims.
#[derive(Debug, Clone)]
pub struct VerifiedIdentity {
    pub email: String,
    #[allow(dead_code)] // returned for future use/enforcement; used in lib tests
    pub sub: String,
}

#[derive(Debug)]
pub enum JwtError {
    MissingAssertion,
    JwksLoad(String),
    KeyNotFound,
    Decode(String),
    InvalidIssuer,
    InvalidAudience,
    Expired,
    Immature,
    MissingEmail,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::MissingAssertion => write!(f, "Cf-Access-Jwt-Assertion header missing"),
            JwtError::JwksLoad(e) => write!(f, "JWKS load failed: {e}"),
            JwtError::KeyNotFound => write!(f, "no JWKS key matched the token kid"),
            JwtError::Decode(e) => write!(f, "JWT signature/decode failed: {e}"),
            JwtError::InvalidIssuer => write!(f, "JWT iss not the team domain"),
            JwtError::InvalidAudience => write!(f, "JWT aud not in allowed set"),
            JwtError::Expired => write!(f, "JWT has expired"),
            JwtError::Immature => write!(f, "JWT not yet valid (iat in future)"),
            JwtError::MissingEmail => write!(f, "JWT has no email claim"),
        }
    }
}

impl std::error::Error for JwtError {}

/// Configuration for CF Access JWT verification (from config.rs / core.env).
#[derive(Debug, Clone)]
pub struct CfJwtConfig {
    pub team_domain: String,
    pub aud: HashSet<String>,
    pub jwks_path: PathBuf,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CfClaims {
    #[serde(rename = "iss")]
    iss: Option<String>,
    #[serde(rename = "aud")]
    aud: Option<serde_json::Value>,
    exp: usize,
    #[serde(rename = "iat")]
    iat: Option<usize>,
    email: Option<String>,
    sub: Option<String>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: Option<String>,
    #[serde(rename = "n")]
    n: String,
    #[serde(rename = "e")]
    e: String,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

const ASSERTION_HEADER: &str = "Cf-Access-Jwt-Assertion";
const LEEWAY_SECS: u64 = 60;

#[cfg(test)]
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn load_jwks(path: &Path) -> Result<Jwks, JwtError> {
    let content = std::fs::read_to_string(path).map_err(|e| JwtError::JwksLoad(e.to_string()))?;
    serde_json::from_str(&content).map_err(|e| JwtError::JwksLoad(e.to_string()))
}

fn map_decode_error(e: jsonwebtoken::errors::Error) -> JwtError {
    use jsonwebtoken::errors::ErrorKind;
    match e.kind() {
        ErrorKind::ExpiredSignature => JwtError::Expired,
        ErrorKind::ImmatureSignature => JwtError::Immature,
        ErrorKind::InvalidAudience => JwtError::InvalidAudience,
        ErrorKind::InvalidIssuer => JwtError::InvalidIssuer,
        _ => JwtError::Decode(e.to_string()),
    }
}

/// Verify the `Cf-Access-Jwt-Assertion` in `headers` against the pinned JWKS and
/// the configured team/audience. Returns the verified identity on success.
pub fn verify_assertion(
    headers: &HeaderMap,
    cfg: &CfJwtConfig,
) -> Result<VerifiedIdentity, JwtError> {
    let assertion = headers
        .get(ASSERTION_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(JwtError::MissingAssertion)?;

    // Decode header to discover the key id (kid) for JWKS matching.
    let header =
        jsonwebtoken::decode_header(assertion).map_err(|e| JwtError::Decode(e.to_string()))?;
    let kid = header.kid.ok_or(JwtError::KeyNotFound)?;

    let jwks = load_jwks(&cfg.jwks_path)?;
    let key = jwks
        .keys
        .iter()
        .find(|k| k.kid.as_deref() == Some(&kid))
        .ok_or(JwtError::KeyNotFound)?;

    let decoding_key = jsonwebtoken::DecodingKey::from_rsa_components(&key.n, &key.e)
        .map_err(|e| JwtError::JwksLoad(e.to_string()))?;

    // Verify signature + validate iss/aud/exp (with leeway) via jsonwebtoken.
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.leeway = LEEWAY_SECS;
    let aud_list: Vec<&str> = cfg.aud.iter().map(|s| s.as_str()).collect();
    validation.set_audience(&aud_list);
    validation.set_issuer(&[cfg.team_domain.as_str()]);

    let token_data = jsonwebtoken::decode::<CfClaims>(assertion, &decoding_key, &validation)
        .map_err(map_decode_error)?;
    let claims = token_data.claims;

    let email = claims.email.ok_or(JwtError::MissingEmail)?;
    Ok(VerifiedIdentity {
        email,
        sub: claims.sub.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use jsonwebtoken::EncodingKey;
    use rsa::traits::PublicKeyParts;
    use std::collections::HashSet;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    const KID: &str = "p5test";
    const TEAM: &str = "https://nrupal.cloudflareaccess.com";

    fn b64url(data: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
    }

    // Generate a fresh RSA keypair + JWKS at runtime (never embed a private key in
    // source). Shared by the verifier's JWKS and the token issuer's signing key.
    struct TestEnv {
        cfg: CfJwtConfig,
        enc: EncodingKey,
    }

    fn setup() -> TestEnv {
        let mut rng = rand::thread_rng();
        let private = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public = private.to_public_key();
        let n = public.n().to_bytes_be();
        let e = public.e().to_bytes_be();
        let jwks = serde_json::json!({
            "keys": [{
                "kty": "RSA", "use": "sig", "alg": "RS256", "kid": KID,
                "n": b64url(&n), "e": b64url(&e)
            }]
        });
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("p5test{}_{}", std::process::id(), seq));
        fs::create_dir_all(&dir).ok();
        let jwks_path = dir.join("jwks.json");
        fs::write(&jwks_path, jwks.to_string()).ok();

        let pem = rsa::pkcs8::EncodePrivateKey::to_pkcs8_pem(&private, rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string();
        let enc = EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap();

        let mut aud = HashSet::new();
        aud.insert("aud-x".to_string());
        let cfg = CfJwtConfig {
            team_domain: TEAM.to_string(),
            aud,
            jwks_path,
            enabled: true,
        };
        TestEnv { cfg, enc }
    }

    fn sign(env: &TestEnv, claims: serde_json::Value) -> String {
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(KID.to_string());
        jsonwebtoken::encode(&header, &claims, &env.enc).unwrap()
    }

    fn headers(token: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(t) = token {
            h.insert(ASSERTION_HEADER, t.parse().unwrap());
        }
        h
    }

    fn valid_claims(exp_offset: i64) -> serde_json::Value {
        let now = now_secs() as i64;
        serde_json::json!({
            "iss": TEAM,
            "aud": "aud-x",
            "exp": now + exp_offset,
            "iat": now,
            "email": "admin@example.com",
            "sub": "user-123"
        })
    }

    #[test]
    fn valid_assertion_yields_identity() {
        let env = setup();
        let h = headers(Some(&sign(&env, valid_claims(3600))));
        let id = verify_assertion(&h, &env.cfg)
            .unwrap_or_else(|e| panic!("valid token rejected: {e:?}"));
        assert_eq!(id.email, "admin@example.com");
        assert_eq!(id.sub, "user-123");
    }

    #[test]
    fn bad_signature_errors() {
        let env = setup();
        let mut token = sign(&env, valid_claims(3600));
        let len = token.len();
        let replacement = if token.bytes().last() == Some(b'a') {
            "b"
        } else {
            "a"
        };
        token.replace_range(len - 1.., replacement);
        let h = headers(Some(&token));
        assert!(matches!(
            verify_assertion(&h, &env.cfg),
            Err(JwtError::Decode(_))
        ));
    }

    #[test]
    fn wrong_audience_errors() {
        let env = setup();
        let mut claims = valid_claims(3600);
        claims["aud"] = serde_json::json!("aud-other");
        let h = headers(Some(&sign(&env, claims)));
        assert!(matches!(
            verify_assertion(&h, &env.cfg),
            Err(JwtError::InvalidAudience)
        ));
    }

    #[test]
    fn expired_errors() {
        let env = setup();
        let h = headers(Some(&sign(&env, valid_claims(-7200))));
        assert!(matches!(
            verify_assertion(&h, &env.cfg),
            Err(JwtError::Expired)
        ));
    }

    #[test]
    fn wrong_issuer_errors() {
        let env = setup();
        let mut claims = valid_claims(3600);
        claims["iss"] = serde_json::json!("https://evil.example.com");
        let h = headers(Some(&sign(&env, claims)));
        assert!(matches!(
            verify_assertion(&h, &env.cfg),
            Err(JwtError::InvalidIssuer)
        ));
    }

    #[test]
    fn missing_assertion_errors() {
        let env = setup();
        let h = headers(None);
        assert!(matches!(
            verify_assertion(&h, &env.cfg),
            Err(JwtError::MissingAssertion)
        ));
    }
}
