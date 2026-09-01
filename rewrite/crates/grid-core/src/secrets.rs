use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("secret encoding: {0}")]
    Encoding(String),
}

/// A server credential. Debug is derived but the SecretString fields render
/// as redacted by the secrecy crate, so this type is safe to log by accident.
#[derive(Debug, Clone)]
pub enum Credential {
    Token(SecretString),
    Basic {
        username: String,
        password: SecretString,
    },
}

pub trait SecretStore: Send + Sync {
    fn save(&self, cred: &Credential) -> Result<(), SecretError>;
    fn load(&self) -> Result<Option<Credential>, SecretError>;
    fn clear(&self) -> Result<(), SecretError>;
}

const SERVICE: &str = "grid-launcher";
const ACCOUNT: &str = "romm-credential";

/// Serialized form kept ONLY inside the OS keyring item.
#[derive(serde::Serialize, serde::Deserialize)]
enum StoredCredential {
    Token { token: String },
    Basic { username: String, password: String },
}

pub struct KeyringStore;

impl KeyringStore {
    pub fn new() -> Self {
        Self
    }
    fn entry() -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| SecretError::Keyring(e.to_string()))
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringStore {
    fn save(&self, cred: &Credential) -> Result<(), SecretError> {
        let stored = match cred {
            Credential::Token(t) => StoredCredential::Token {
                token: t.expose_secret().to_string(),
            },
            Credential::Basic { username, password } => StoredCredential::Basic {
                username: username.clone(),
                password: password.expose_secret().to_string(),
            },
        };
        let json =
            serde_json::to_string(&stored).map_err(|e| SecretError::Encoding(e.to_string()))?;
        Self::entry()?
            .set_password(&json)
            .map_err(|e| SecretError::Keyring(e.to_string()))
    }

    fn load(&self) -> Result<Option<Credential>, SecretError> {
        let json = match Self::entry()?.get_password() {
            Ok(j) => j,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(SecretError::Keyring(e.to_string())),
        };
        let stored: StoredCredential =
            serde_json::from_str(&json).map_err(|e| SecretError::Encoding(e.to_string()))?;
        Ok(Some(match stored {
            StoredCredential::Token { token } => Credential::Token(SecretString::from(token)),
            StoredCredential::Basic { username, password } => Credential::Basic {
                username,
                password: SecretString::from(password),
            },
        }))
    }

    fn clear(&self) -> Result<(), SecretError> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::Keyring(e.to_string())),
        }
    }
}

/// In-memory store for tests and for the Tauri layer's unit tests.
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<Option<Credential>>);

impl SecretStore for MemoryStore {
    fn save(&self, cred: &Credential) -> Result<(), SecretError> {
        *self.0.lock().unwrap() = Some(cred.clone());
        Ok(())
    }
    fn load(&self) -> Result<Option<Credential>, SecretError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn clear(&self) -> Result<(), SecretError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    #[test]
    fn debug_output_redacts_secrets() {
        let cred = Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"));
        let debug = format!("{cred:?}");
        assert!(!debug.contains("FAKE-TEST-TOKEN-not-real"), "leak: {debug}");
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryStore::default();
        store
            .save(&Credential::Basic {
                username: "six".into(),
                password: SecretString::from("pw-FAKE"),
            })
            .unwrap();
        match store.load().unwrap() {
            Some(Credential::Basic { username, .. }) => assert_eq!(username, "six"),
            other => panic!("wrong credential: {other:?}"),
        }
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }
}
