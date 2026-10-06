//! API keys. They are kept in the operating system's credential store
//! (macOS Keychain, Windows Credential Manager), one entry per provider, and
//! are read only by Rust, only to make requests. The UI is told whether a key
//! is saved, never the key.

use nimata_core::domain::{ProviderConfig, ProviderKind};
use serde::Serialize;

/// Service name under which keys are stored in the credential store.
pub const SERVICE: &str = "org.nimata.app";

/// Environment variables read for keys in development builds.
pub const OPENAI_ENV: &str = "OPENAI_API_KEY";
pub const ANTHROPIC_ENV: &str = "ANTHROPIC_API_KEY";

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// The operating system's credential store.
pub struct OsSecretStore;

#[cfg(not(target_os = "android"))]
impl SecretStore for OsSecretStore {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("could not read the key from {}: {e}", store_name())),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, account)
            .and_then(|entry| entry.set_password(secret))
            .map_err(|e| format!("could not save the key in {}: {e}", store_name()))
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!(
                "could not remove the key from {}: {e}",
                store_name()
            )),
        }
    }
}

#[cfg(target_os = "android")]
impl SecretStore for OsSecretStore {
    fn get(&self, _account: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn set(&self, _account: &str, _secret: &str) -> Result<(), String> {
        Err("API keys cannot be saved on Android yet.".into())
    }

    fn delete(&self, _account: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The name of the credential store, as people know it on this platform.
pub fn store_name() -> &'static str {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        "the Keychain"
    } else if cfg!(target_os = "windows") {
        "Windows Credential Manager"
    } else {
        "the system keyring"
    }
}

/// An in-memory store for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemorySecretStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
impl SecretStore for MemorySecretStore {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(account.into(), secret.into());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(account);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeySource {
    /// Saved in the credential store.
    Saved,
    /// From an environment variable (development builds only).
    Environment,
}

/// What the UI may know about a provider's key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub source: Option<KeySource>,
    /// The key's last four characters, in development builds only.
    pub hint: Option<String>,
    /// Where saved keys are kept, e.g. "the Keychain".
    pub store: &'static str,
    /// The environment variable read in development builds, if any.
    pub environment_variable: Option<&'static str>,
}

pub struct Keys {
    store: Box<dyn SecretStore>,
    /// Development conveniences: key hints and the environment fallback.
    /// On in debug builds, off in release builds.
    development: bool,
    env: fn(&str) -> Option<String>,
}

fn from_environment(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl Keys {
    pub fn os() -> Self {
        Self::new(
            Box::new(OsSecretStore),
            cfg!(debug_assertions),
            from_environment,
        )
    }

    pub fn new(
        store: Box<dyn SecretStore>,
        development: bool,
        env: fn(&str) -> Option<String>,
    ) -> Self {
        Self {
            store,
            development,
            env,
        }
    }

    fn account(provider: &ProviderConfig) -> String {
        format!("provider:{}", provider.id)
    }

    fn environment_variable(&self, provider: &ProviderConfig) -> Option<&'static str> {
        if !self.development {
            return None;
        }
        match provider.kind {
            ProviderKind::OpenAi => Some(OPENAI_ENV),
            ProviderKind::Anthropic => Some(ANTHROPIC_ENV),
            ProviderKind::OpenAiCompatible => None,
        }
    }

    /// The key to use for requests, and where it came from. A saved key
    /// takes precedence over the environment.
    pub fn resolve(
        &self,
        provider: &ProviderConfig,
    ) -> Result<Option<(String, KeySource)>, String> {
        if let Some(key) = self.store.get(&Self::account(provider))? {
            return Ok(Some((key, KeySource::Saved)));
        }
        Ok(self
            .environment_variable(provider)
            .and_then(self.env)
            .map(|key| (key, KeySource::Environment)))
    }

    pub fn status(&self, provider: &ProviderConfig) -> Result<KeyStatus, String> {
        let resolved = self.resolve(provider)?;
        let hint = match &resolved {
            Some((key, _)) if self.development => {
                let chars: Vec<char> = key.chars().collect();
                let start = chars.len().saturating_sub(4);
                Some(chars[start..].iter().collect())
            }
            _ => None,
        };
        Ok(KeyStatus {
            source: resolved.map(|(_, source)| source),
            hint,
            store: store_name(),
            environment_variable: self.environment_variable(provider),
        })
    }

    pub fn save(&self, provider: &ProviderConfig, key: &str) -> Result<KeyStatus, String> {
        let key = key.trim();
        if key.is_empty() {
            return Err("paste the API key first".into());
        }
        if key.chars().any(char::is_whitespace) {
            return Err("an API key cannot contain spaces or line breaks".into());
        }
        self.store.set(&Self::account(provider), key)?;
        self.status(provider)
    }

    pub fn remove(&self, provider: &ProviderConfig) -> Result<KeyStatus, String> {
        self.store.delete(&Self::account(provider))?;
        self.status(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn openai() -> ProviderConfig {
        ProviderConfig {
            id: Uuid::now_v7(),
            kind: ProviderKind::OpenAi,
            display_name: "OpenAI".into(),
            base_url: None,
        }
    }

    fn env_with_key(name: &str) -> Option<String> {
        (name == OPENAI_ENV).then(|| "sk-from-environment-wxyz".to_string())
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_saved_key_is_used_and_only_hinted_at_in_development() {
        let provider = openai();
        let keys = Keys::new(Box::new(MemorySecretStore::default()), true, no_env);
        let status = keys.save(&provider, "  sk-proj-abcd1234  ").unwrap();
        assert_eq!(status.source, Some(KeySource::Saved));
        assert_eq!(status.hint.as_deref(), Some("1234"));
        assert_eq!(
            keys.resolve(&provider).unwrap(),
            Some(("sk-proj-abcd1234".into(), KeySource::Saved))
        );
    }

    #[test]
    fn release_builds_show_no_part_of_the_key_and_ignore_the_environment() {
        let provider = openai();
        let keys = Keys::new(Box::new(MemorySecretStore::default()), false, env_with_key);
        let status = keys.status(&provider).unwrap();
        assert_eq!(status.source, None, "the environment is not read");
        assert_eq!(status.environment_variable, None);

        let status = keys.save(&provider, "sk-proj-abcd1234").unwrap();
        assert_eq!(status.source, Some(KeySource::Saved));
        assert_eq!(status.hint, None, "no part of the key is shown");
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("1234"), "{json}");
    }

    #[test]
    fn development_builds_fall_back_to_the_environment() {
        let provider = openai();
        let keys = Keys::new(Box::new(MemorySecretStore::default()), true, env_with_key);
        let status = keys.status(&provider).unwrap();
        assert_eq!(status.source, Some(KeySource::Environment));
        assert_eq!(status.environment_variable, Some(OPENAI_ENV));
        assert_eq!(status.hint.as_deref(), Some("wxyz"));

        keys.save(&provider, "sk-saved-9999").unwrap();
        assert_eq!(
            keys.resolve(&provider).unwrap(),
            Some(("sk-saved-9999".into(), KeySource::Saved)),
            "a saved key wins"
        );
        keys.remove(&provider).unwrap();
        assert_eq!(
            keys.status(&provider).unwrap().source,
            Some(KeySource::Environment)
        );
    }

    #[test]
    fn each_kind_reads_its_own_environment_variable() {
        fn env(name: &str) -> Option<String> {
            match name {
                ANTHROPIC_ENV => Some("sk-ant-env-cdef".into()),
                OPENAI_ENV => Some("sk-openai-env-abcd".into()),
                _ => None,
            }
        }
        let keys = Keys::new(Box::new(MemorySecretStore::default()), true, env);
        let mut anthropic = openai();
        anthropic.kind = ProviderKind::Anthropic;
        assert_eq!(
            keys.status(&anthropic).unwrap().environment_variable,
            Some(ANTHROPIC_ENV)
        );
        assert_eq!(
            keys.status(&anthropic).unwrap().hint.as_deref(),
            Some("cdef")
        );

        let mut local = openai();
        local.kind = ProviderKind::OpenAiCompatible;
        assert_eq!(
            keys.status(&local).unwrap().source,
            None,
            "no environment fallback for custom endpoints"
        );
    }

    #[test]
    fn malformed_keys_are_rejected_before_saving() {
        let provider = openai();
        let keys = Keys::new(Box::new(MemorySecretStore::default()), true, no_env);
        assert!(keys.save(&provider, "   ").is_err());
        assert!(keys.save(&provider, "sk-abc\nsk-def").is_err());
        assert_eq!(keys.status(&provider).unwrap().source, None);
    }

    #[test]
    fn each_provider_has_its_own_key() {
        let (a, b) = (openai(), openai());
        let keys = Keys::new(Box::new(MemorySecretStore::default()), true, no_env);
        keys.save(&a, "sk-aaaa").unwrap();
        assert_eq!(keys.status(&b).unwrap().source, None);
    }
}
