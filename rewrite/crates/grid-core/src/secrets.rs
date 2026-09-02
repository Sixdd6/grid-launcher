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
/// A SECOND, independent keyring item under the same [`SERVICE`]: clearing
/// the RomM credential (`ACCOUNT`) must never clear the RetroAchievements
/// token, and vice versa — they are two accounts, not one shared slot.
const RA_ACCOUNT: &str = "retroachievements-token";

/// The RetroAchievements token's OS-keyring slot. A separate trait from
/// [`SecretStore`] (rather than a third `Credential` variant) because it is
/// a second, independent keyring item — see [`RA_ACCOUNT`].
pub trait RaTokenStore: Send + Sync {
    fn save(&self, token: &SecretString) -> Result<(), SecretError>;
    fn load(&self) -> Result<Option<SecretString>, SecretError>;
    fn clear(&self) -> Result<(), SecretError>;
}

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
    fn ra_entry() -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, RA_ACCOUNT).map_err(|e| SecretError::Keyring(e.to_string()))
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

impl RaTokenStore for KeyringStore {
    fn save(&self, token: &SecretString) -> Result<(), SecretError> {
        Self::ra_entry()?
            .set_password(token.expose_secret())
            .map_err(|e| SecretError::Keyring(e.to_string()))
    }

    fn load(&self) -> Result<Option<SecretString>, SecretError> {
        match Self::ra_entry()?.get_password() {
            Ok(token) => Ok(Some(SecretString::from(token))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError::Keyring(e.to_string())),
        }
    }

    fn clear(&self) -> Result<(), SecretError> {
        match Self::ra_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::Keyring(e.to_string())),
        }
    }
}

/// In-memory store for tests and for the Tauri layer's unit tests. The RA
/// token lives in its own field — a second, independent slot mirroring the
/// two real keyring items (see [`RA_ACCOUNT`]) — so clearing one credential
/// here can never clear the other.
#[derive(Default)]
pub struct MemoryStore {
    credential: std::sync::Mutex<Option<Credential>>,
    ra_token: std::sync::Mutex<Option<SecretString>>,
}

impl SecretStore for MemoryStore {
    fn save(&self, cred: &Credential) -> Result<(), SecretError> {
        *self.credential.lock().unwrap() = Some(cred.clone());
        Ok(())
    }
    fn load(&self) -> Result<Option<Credential>, SecretError> {
        Ok(self.credential.lock().unwrap().clone())
    }
    fn clear(&self) -> Result<(), SecretError> {
        *self.credential.lock().unwrap() = None;
        Ok(())
    }
}

impl RaTokenStore for MemoryStore {
    fn save(&self, token: &SecretString) -> Result<(), SecretError> {
        *self.ra_token.lock().unwrap() = Some(token.clone());
        Ok(())
    }
    fn load(&self) -> Result<Option<SecretString>, SecretError> {
        Ok(self.ra_token.lock().unwrap().clone())
    }
    fn clear(&self) -> Result<(), SecretError> {
        *self.ra_token.lock().unwrap() = None;
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
        SecretStore::save(
            &store,
            &Credential::Basic {
                username: "six".into(),
                password: SecretString::from("pw-FAKE"),
            },
        )
        .unwrap();
        match SecretStore::load(&store).unwrap() {
            Some(Credential::Basic { username, .. }) => assert_eq!(username, "six"),
            other => panic!("wrong credential: {other:?}"),
        }
        SecretStore::clear(&store).unwrap();
        assert!(SecretStore::load(&store).unwrap().is_none());
    }

    /// The RA token slot is a SECOND, independent keyring item: saving a
    /// RomM credential and an RA token both leave both readable, and
    /// clearing one must not clear the other.
    #[test]
    fn ra_token_store_round_trips_independently_of_the_romm_credential() {
        let store = MemoryStore::default();
        SecretStore::save(
            &store,
            &Credential::Basic {
                username: "six".into(),
                password: SecretString::from("pw-FAKE"),
            },
        )
        .unwrap();
        RaTokenStore::save(&store, &SecretString::from("FAKE-RA-TOKEN-not-real")).unwrap();

        match SecretStore::load(&store).unwrap() {
            Some(Credential::Basic { username, .. }) => assert_eq!(username, "six"),
            other => panic!("wrong credential: {other:?}"),
        }
        assert_eq!(
            RaTokenStore::load(&store)
                .unwrap()
                .map(|t| t.expose_secret().to_string()),
            Some("FAKE-RA-TOKEN-not-real".to_string())
        );

        // Clearing the RomM credential must not touch the RA token.
        SecretStore::clear(&store).unwrap();
        assert!(SecretStore::load(&store).unwrap().is_none());
        assert!(RaTokenStore::load(&store).unwrap().is_some());

        // And vice versa: clearing the RA token must not touch the RomM
        // credential.
        SecretStore::save(
            &store,
            &Credential::Basic {
                username: "six".into(),
                password: SecretString::from("pw-FAKE"),
            },
        )
        .unwrap();
        RaTokenStore::clear(&store).unwrap();
        assert!(RaTokenStore::load(&store).unwrap().is_none());
        assert!(SecretStore::load(&store).unwrap().is_some());
    }
}
