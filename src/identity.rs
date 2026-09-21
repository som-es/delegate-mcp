use serde_json::Value;
use sha3::{Digest, Sha3_256};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct McpIdentity(pub String);

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn normalize_user_id(raw: Option<&[u8]>) -> Option<String> {
    let value = String::from_utf8_lossy(raw?).trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub fn identity_from_stored(stored: Option<&[u8]>, hashed_token: &str) -> McpIdentity {
    match normalize_user_id(stored).filter(|v| v != hashed_token) {
        Some(user_id) => McpIdentity(user_id),
        None => McpIdentity(hashed_token.to_string()),
    }
}

pub fn user_id_from_claims(claims: &Value) -> Option<String> {
    ["sub", "user_id", "uid", "id"].iter().find_map(|key| {
        claims.get(*key).and_then(|v| match v {
            Value::String(s) => normalize_user_id(Some(s.as_bytes())),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bytes(v: &str) -> Vec<u8> {
        v.as_bytes().to_vec()
    }

    #[test]
    fn legacy_empty_value_falls_back_to_token_hash() {
        let hashed = hash_token("mcp_abc");
        assert_eq!(
            identity_from_stored(Some(&bytes("")), &hashed),
            McpIdentity(hashed.clone())
        );
        assert_eq!(identity_from_stored(None, &hashed), McpIdentity(hashed));
    }

    #[test]
    fn stored_user_id_is_used_when_present() {
        let hashed = hash_token("mcp_abc");
        assert_eq!(
            identity_from_stored(Some(&bytes("42")), &hashed),
            McpIdentity("42".to_string())
        );
        assert_eq!(
            identity_from_stored(Some(&bytes("  someone@example ")), &hashed),
            McpIdentity("someone@example".to_string())
        );
    }

    #[test]
    fn user_id_is_extracted_from_claims_as_string_or_int() {
        assert_eq!(
            user_id_from_claims(&json!({"sub": "42"})),
            Some("42".to_string())
        );
        assert_eq!(
            user_id_from_claims(&json!({"sub": 42})),
            Some("42".to_string())
        );
        assert_eq!(
            user_id_from_claims(&json!({"user_id": 7, "sub": ""})),
            Some("7".to_string())
        );
        assert_eq!(user_id_from_claims(&json!({"scope": "mcp:read"})), None);
    }
}
