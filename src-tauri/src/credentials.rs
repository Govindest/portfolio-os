//! Credential storage. Secrets live only in the operating-system credential
//! store (macOS Keychain / Windows Credential Manager / Linux Secret Service)
//! keyed by connection id. Nothing secret ever touches SQLite, logs, or the
//! frontend after initial entry.

use crate::errors::BrokerError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

const SERVICE: &str = "com.portfolio-os.credentials";

/// Secret material for a brokerage connection, serialized as JSON in the
/// credential store entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrokerSecrets {
    /// Alpaca API key id, Tradier token, etc.
    pub key: String,
    /// Alpaca API secret; empty for token-based brokers.
    #[serde(default)]
    pub secret: String,
}

pub trait CredentialStore: Send + Sync {
    fn save(&self, connection_id: &str, secrets: &BrokerSecrets) -> Result<(), BrokerError>;
    fn get(&self, connection_id: &str) -> Result<Option<BrokerSecrets>, BrokerError>;
    fn delete(&self, connection_id: &str) -> Result<(), BrokerError>;
}

/// Production store backed by the OS keychain.
pub struct KeyringStore;

impl KeyringStore {
    fn entry(connection_id: &str) -> Result<keyring::Entry, BrokerError> {
        keyring::Entry::new(SERVICE, connection_id)
            .map_err(|e| BrokerError::credential_store(e.to_string()))
    }
}

impl CredentialStore for KeyringStore {
    fn save(&self, connection_id: &str, secrets: &BrokerSecrets) -> Result<(), BrokerError> {
        let json = serde_json::to_string(secrets)
            .map_err(|e| BrokerError::credential_store(e.to_string()))?;
        Self::entry(connection_id)?
            .set_password(&json)
            .map_err(|e| BrokerError::credential_store(e.to_string()))
    }

    fn get(&self, connection_id: &str) -> Result<Option<BrokerSecrets>, BrokerError> {
        match Self::entry(connection_id)?.get_password() {
            Ok(json) => serde_json::from_str(&json)
                .map(Some)
                .map_err(|e| BrokerError::credential_store(e.to_string())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(BrokerError::credential_store(e.to_string())),
        }
    }

    fn delete(&self, connection_id: &str) -> Result<(), BrokerError> {
        match Self::entry(connection_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(BrokerError::credential_store(e.to_string())),
        }
    }
}

/// In-memory store for automated tests — same contract, no OS keychain.
#[derive(Default)]
pub struct MemoryStore {
    map: Mutex<HashMap<String, BrokerSecrets>>,
}

impl CredentialStore for MemoryStore {
    fn save(&self, connection_id: &str, secrets: &BrokerSecrets) -> Result<(), BrokerError> {
        self.map
            .lock()
            .unwrap()
            .insert(connection_id.into(), secrets.clone());
        Ok(())
    }
    fn get(&self, connection_id: &str) -> Result<Option<BrokerSecrets>, BrokerError> {
        Ok(self.map.lock().unwrap().get(connection_id).cloned())
    }
    fn delete(&self, connection_id: &str) -> Result<(), BrokerError> {
        self.map.lock().unwrap().remove(connection_id);
        Ok(())
    }
}

/// Mask a credential for display: keep the first 4 characters at most.
pub fn mask(value: &str) -> String {
    let visible: String = value.chars().take(4).collect();
    format!("{visible}{}", "•".repeat(8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip_and_delete() {
        let store = MemoryStore::default();
        let secrets = BrokerSecrets {
            key: "PK123".into(),
            secret: "shh".into(),
        };
        store.save("conn-1", &secrets).unwrap();
        let loaded = store.get("conn-1").unwrap().unwrap();
        assert_eq!(loaded.key, "PK123");
        assert_eq!(loaded.secret, "shh");
        store.delete("conn-1").unwrap();
        assert!(store.get("conn-1").unwrap().is_none());
        // Deleting twice is fine.
        store.delete("conn-1").unwrap();
    }

    #[test]
    fn mask_hides_secret() {
        assert_eq!(mask("PKABCDEFG"), "PKAB••••••••");
        assert!(!mask("supersecretvalue").contains("secret"));
    }
}
