//! ed25519 signatures for manifests and catalog items (ADR-007). The private key never leaves the publisher's
//! PC; the launcher embeds the public key.
//!
//! - Manifests: signature over the exact (uncompressed) manifest bytes.
//! - Catalog items: signature over [`canonical_json`] of the item with its `sig` field removed. Canonical form
//!   makes the signature independent of key order / whitespace, so the server may re-serialize items freely.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Signer, Verifier};
pub use ed25519_dalek::{SigningKey, VerifyingKey};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("invalid key: {0}")]
    Key(String),
    #[error("invalid signature encoding")]
    Encoding,
    #[error("signature does not match")]
    Mismatch,
    #[error("item has no signature")]
    Missing,
}

/// Secret key file format: base64 of the 32-byte seed, one line.
pub fn signing_key_from_b64(s: &str) -> Result<SigningKey, SignError> {
    let bytes = B64.decode(s.trim()).map_err(|e| SignError::Key(e.to_string()))?;
    let seed: [u8; 32] = bytes.try_into().map_err(|_| SignError::Key("expected 32 bytes".into()))?;
    Ok(SigningKey::from_bytes(&seed))
}

pub fn signing_key_to_b64(k: &SigningKey) -> String {
    B64.encode(k.to_bytes())
}

/// Public key format: base64 of the 32-byte verifying key.
pub fn verifying_key_from_b64(s: &str) -> Result<VerifyingKey, SignError> {
    let bytes = B64.decode(s.trim()).map_err(|e| SignError::Key(e.to_string()))?;
    let arr: [u8; 32] = bytes.try_into().map_err(|_| SignError::Key("expected 32 bytes".into()))?;
    VerifyingKey::from_bytes(&arr).map_err(|e| SignError::Key(e.to_string()))
}

pub fn verifying_key_to_b64(k: &VerifyingKey) -> String {
    B64.encode(k.to_bytes())
}

pub fn sign_bytes(key: &SigningKey, data: &[u8]) -> String {
    B64.encode(key.sign(data).to_bytes())
}

pub fn verify_bytes(key: &VerifyingKey, data: &[u8], sig_b64: &str) -> Result<(), SignError> {
    let raw = B64.decode(sig_b64.trim()).map_err(|_| SignError::Encoding)?;
    let arr: [u8; 64] = raw.try_into().map_err(|_| SignError::Encoding)?;
    key.verify(data, &Signature::from_bytes(&arr)).map_err(|_| SignError::Mismatch)
}

/// Deterministic JSON: object keys sorted (byte order), no whitespace. Independent of serde_json's
/// `preserve_order` feature, which other crates in the workspace may switch on.
pub fn canonical_json(v: &Value) -> String {
    let mut out = String::new();
    write_canonical(v, &mut out);
    out
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).expect("string serializes"));
                out.push(':');
                write_canonical(&map[k], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).expect("scalar serializes")),
    }
}

/// Signs a catalog item in place (`item["sig"]`).
pub fn sign_item(key: &SigningKey, item: &mut Value) {
    if let Value::Object(map) = item {
        map.remove("sig");
    }
    let sig = sign_bytes(key, canonical_json(item).as_bytes());
    if let Value::Object(map) = item {
        map.insert("sig".into(), Value::String(sig));
    }
}

/// Verifies a catalog item's `sig` field.
pub fn verify_item(key: &VerifyingKey, item: &Value) -> Result<(), SignError> {
    let mut copy = item.clone();
    let sig = match copy.as_object_mut().and_then(|m| m.remove("sig")) {
        Some(Value::String(s)) => s,
        _ => return Err(SignError::Missing),
    };
    verify_bytes(key, canonical_json(&copy).as_bytes(), &sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn canonical_sorts_keys_recursively() {
        let v = json!({ "b": 1, "a": { "z": [2, { "y": true, "x": null }], "c": "é\"" } });
        assert_eq!(canonical_json(&v), r#"{"a":{"c":"é\"","z":[2,{"x":null,"y":true}]},"b":1}"#);
    }

    #[test]
    fn item_signature_survives_reordering_and_detects_tampering() {
        let k = key();
        let mut item = json!({ "id": "gracidea", "name": "Pokemon Gracidea", "channels": { "stable": { "version": "1" } } });
        sign_item(&k, &mut item);
        // Same item, different key order and whitespace (as a server might re-serialize it).
        let sig = item["sig"].as_str().unwrap();
        let reordered = format!(
            r#"{{ "sig": "{sig}", "channels": {{ "stable": {{ "version": "1" }} }}, "name": "Pokemon Gracidea", "id": "gracidea" }}"#
        );
        let reparsed: Value = serde_json::from_str(&reordered).unwrap();
        verify_item(&k.verifying_key(), &reparsed).unwrap();

        let mut tampered = item.clone();
        tampered["channels"]["stable"]["version"] = json!("2");
        assert!(matches!(verify_item(&k.verifying_key(), &tampered), Err(SignError::Mismatch)));
    }

    #[test]
    fn key_roundtrip() {
        let k = key();
        let k2 = signing_key_from_b64(&signing_key_to_b64(&k)).unwrap();
        assert_eq!(k.to_bytes(), k2.to_bytes());
        let pk = verifying_key_from_b64(&verifying_key_to_b64(&k.verifying_key())).unwrap();
        let sig = sign_bytes(&k, b"hello");
        verify_bytes(&pk, b"hello", &sig).unwrap();
        assert!(verify_bytes(&pk, b"hellO", &sig).is_err());
    }
}
