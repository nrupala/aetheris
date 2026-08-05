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
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use std::collections::HashSet;
    use std::fs;

    // Test RSA key `kid=p5test`; private key signs tokens, public n/e go into a temp
    // JWKS file written by each test.
    const KID: &str = "p5test";
    const RSA_N: &str = "hHNlY41eLuZQLAiVBadSiOBf2j0LmtMezIAMCN05-9tQ0y9w6q-bwd2znboBBFheKTuB6pQSXvtV473OmrS0IKi2nJpFYcMZXdplbz9EZsx65UqZcrahwS4SwBuJ9AIdkB1Vboqa5Mv-B6qOeBxyg3TU0ynbZ639uvD8ZkGPOAOf4Rs6lPPaa8X-z0umULYpKTGjsr7MnSXXRcZdZGuedxDFQqcUiTGIcjB8etgyulu1jzgqfdwmGdWbrm6OT6GdKFaYSRx2qvRQJFFfTAAcZRiYM6eqNyPqPZ7cmJLuns3_oZt9PlgOvfhuIBS1uUmoKJULnhzG0zmKd0yxIkq_cw";
    const RSA_E: &str = "AQAB";
    const PRIV_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCEc2VjjV4u5lAs
CJUFp1KI4F/aPQua0x7MgAwI3Tn721DTL3Dqr5vB3bOdugEEWF4pO4HqlBJe+1Xj
vc6atLQgqLacmkVhwxld2mVvP0RmzHrlSplytqHBLhLAG4n0Ah2QHVVuiprky/4H
qo54HHKDdNTTKdtnrf268PxmQY84A5/hGzqU89prxf7PS6ZQtikpMaOyvsydJddF
xl1ka553EMVCpxSJMYhyMHx62DK6W7WPOCp93CYZ1Zuubo5PoZ0oVphJHHaq9FAk
UV9MABxlGJgzp6o3I+o9ntyYku6ezf+hm30+WA69+G4gFLW5SagolQueHMbTOYp3
TLEiSr9zAgMBAAECggEAEkG7vHysekd5zpACRoy5Ri+zVqgqdNVb/fE6d3BTAUHK
QsnbxSWekRrnmrqcUEaD+CgBMN3nKFt5i9JKa62z1Hqc7TogjSiw0ux7JdnxKkBO
QlPFkffVlQSuRfelzNIL1CUO9RU644Wwxsq+J5U4PaF5gn/XA9QKUN3N1KR9wske
WIPTpDvsVezec+fcHtxCDQovq49xm2UyPXhz5q2sT3KQOGEwt0egCwHR8b4ZWieC
4G0UM5rD56ywLhu22A8j50vVZh+qR/cDP8p09UJKcXJJDYvCVYQ9vctb+7mzyFTr
gtFWJ9u6jNfazJwIjTdWSIHsox1kgrLXAcYYWt6uhQKBgQC6euJ8hhqm4kCZn7r+
urNZD1qJS4RIjsyY8uvjTcvsTMHnOfE6IDsAmXnHHb01P0JDjoWBp0JMNMqNsfPz
m4jotaaZIIFOuiNgyi4lwrN0a6oj/9VOv4msDZs8Vz3TIenuneEdE+M9oje5GgWi
WNN0ZS2JswP6Aunv6HJFrwlQxQKBgQC11B2BFcLlhjpbbe4k5QYA6AceGDAf96eY
yoJcTOetX2BZAzcTfEtwb0+R93AspNUXQ13WX7EPfEHaJVjBOPp0P3VF2fMQ8J+l
v0mlND9GQUAFQpsV5sKMO2dQiQn3ZbCQmp+Gdd6b1+I2wqxv95/5Te72sBvFp9Gd
+DqOViLi1wKBgGAcyfMIY2A0KLuFOinkLF/wq+crhuimwQjr22xyQnJuNVpp4Mzm
o8JxV/SqfUSecBbFtEXY4TDJ3MQfPe8G8Q+P4Gf3+u2KvoU6b4KC0V9lxnF7gINv
8RM+iA4XoQPa7OlRch88itjPbQz4PoMoaQQKyee43onTSqOeGJeV2aVJAoGBAJHH
vt//0oKzW5ZyTLzH4khXv10hh3QZ2wVlV58pCZa3IUg8i6vTu6gplmIxQH6KqU49
dL6regowVZvQ1ZgVVrhdKGkYlQi/4z/AXgtWGGT7a5jMDgtBODm2Zt7rAFKZ9TX6
wmvLlO7d50CAVEBxCJGZKj4edCXEpwtAObJk3ROBAoGBAKZ/rjc4+q+nHgfByCK7
TidotvK+qYhEoiw3lznV9IwODZJYgIpn+y/q0xP/kch1OkPmIN57SjBodUf/Jv3G
kLPNsPrriKEpJDFVhahh0emQ+ZWA2NwRG4ketBGvci57BUrFeUpCN68tgDic3r24
rLTURVdg0VsYam7IXDSRDm4Y
-----END PRIVATE KEY-----"#;

    const TEAM: &str = "https://nrupal.cloudflareaccess.com";

    fn cfg() -> CfJwtConfig {
        let mut aud = HashSet::new();
        aud.insert("aud-x".to_string());
        let dir = std::env::temp_dir().join(format!("p5test{}", std::process::id()));
        fs::create_dir_all(&dir).ok();
        let jwks_path = dir.join("jwks.json");
        let jwks = format!(
            r#"{{"keys":[{{"kty":"RSA","use":"sig","alg":"RS256","kid":"{KID}","n":"{RSA_N}","e":"{RSA_E}"}}]}}"#
        );
        fs::write(&jwks_path, jwks).ok();
        CfJwtConfig {
            team_domain: TEAM.to_string(),
            aud,
            jwks_path,
            enabled: true,
        }
    }

    fn sign(claims: serde_json::Value) -> String {
        let key = EncodingKey::from_rsa_pem(PRIV_PEM.as_bytes()).unwrap();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KID.to_string());
        jsonwebtoken::encode(&header, &claims, &key).unwrap()
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
        let c = cfg();
        let h = headers(Some(&sign(valid_claims(3600))));
        let id = verify_assertion(&h, &c).unwrap_or_else(|e| panic!("valid token rejected: {e:?}"));
        assert_eq!(id.email, "admin@example.com");
        assert_eq!(id.sub, "user-123");
    }

    #[test]
    fn bad_signature_errors() {
        let c = cfg();
        let mut token = sign(valid_claims(3600));
        // tamper the last char of the signature
        let len = token.len();
        let replacement = if token.bytes().last() == Some(b'a') {
            "b"
        } else {
            "a"
        };
        token.replace_range(len - 1.., replacement);
        let h = headers(Some(&token));
        assert!(matches!(verify_assertion(&h, &c), Err(JwtError::Decode(_))));
    }

    #[test]
    fn wrong_audience_errors() {
        let c = cfg();
        let mut claims = valid_claims(3600);
        claims["aud"] = serde_json::json!("aud-other");
        let h = headers(Some(&sign(claims)));
        assert!(matches!(
            verify_assertion(&h, &c),
            Err(JwtError::InvalidAudience)
        ));
    }

    #[test]
    fn expired_errors() {
        let c = cfg();
        let h = headers(Some(&sign(valid_claims(-7200))));
        assert!(matches!(verify_assertion(&h, &c), Err(JwtError::Expired)));
    }

    #[test]
    fn wrong_issuer_errors() {
        let c = cfg();
        let mut claims = valid_claims(3600);
        claims["iss"] = serde_json::json!("https://evil.example.com");
        let h = headers(Some(&sign(claims)));
        assert!(matches!(
            verify_assertion(&h, &c),
            Err(JwtError::InvalidIssuer)
        ));
    }

    #[test]
    fn missing_assertion_errors() {
        let c = cfg();
        let h = headers(None);
        assert!(matches!(
            verify_assertion(&h, &c),
            Err(JwtError::MissingAssertion)
        ));
    }
}
