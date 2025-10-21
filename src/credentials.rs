//! Credential management for SSH connections.
//!
//! This module provides secure storage and management of SSH credentials.
//! **Note:** Credentials are only supported for SSH-based connections (SSH, Ping, TCP).
//! HTTP/HTTPS targets do not support credential-based authentication and will open
//! in the default web browser without any credential handling.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Unique identifier for stored credentials
pub type CredentialId = String;

/// Sensitive data that should be securely cleared from memory
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveString(String);

impl SensitiveString {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[allow(dead_code)]
    pub fn into_string(mut self) -> String {
        let result = self.0.clone();
        self.0.zeroize();
        result
    }
}

impl From<String> for SensitiveString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SensitiveString {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

/// Different types of SSH credentials
#[derive(Clone, Serialize, Deserialize)]
pub enum SshCredential {
    /// Use system default SSH behavior (agent, default keys)
    Default,
    /// Username and password authentication
    Password {
        username: String,
        password: SensitiveString,
    },
    /// SSH key authentication
    Key {
        username: String,
        private_key_path: PathBuf,
        passphrase: Option<SensitiveString>,
    },
    /// SSH key with embedded key data
    KeyData {
        username: String,
        private_key_data: SensitiveString,
        passphrase: Option<SensitiveString>,
    },
}

impl SshCredential {
    /// Get the username for this credential, if available
    pub fn username(&self) -> Option<&str> {
        match self {
            SshCredential::Default => None,
            SshCredential::Password { username, .. } => Some(username),
            SshCredential::Key { username, .. } => Some(username),
            SshCredential::KeyData { username, .. } => Some(username),
        }
    }

    /// Check if this credential requires a password/passphrase
    #[allow(dead_code)]
    pub fn requires_secret(&self) -> bool {
        matches!(
            self,
            SshCredential::Password { .. }
                | SshCredential::Key {
                    passphrase: Some(_),
                    ..
                }
                | SshCredential::KeyData {
                    passphrase: Some(_),
                    ..
                }
        )
    }
}

impl Zeroize for SshCredential {
    fn zeroize(&mut self) {
        match self {
            SshCredential::Default => {}
            SshCredential::Password { username, password } => {
                username.zeroize();
                password.zeroize();
            }
            SshCredential::Key {
                username,
                private_key_path: _,
                passphrase,
            } => {
                username.zeroize();
                // Note: PathBuf doesn't implement Zeroize, so we can't zero it
                // This is generally okay since paths are usually not sensitive
                if let Some(passphrase) = passphrase {
                    passphrase.zeroize();
                }
            }
            SshCredential::KeyData {
                username,
                private_key_data,
                passphrase,
            } => {
                username.zeroize();
                private_key_data.zeroize();
                if let Some(passphrase) = passphrase {
                    passphrase.zeroize();
                }
            }
        }
    }
}

impl Drop for SshCredential {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// A stored credential with metadata
#[derive(Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    pub id: CredentialId,
    pub name: String,
    pub description: Option<String>,
    pub credential: SshCredential,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

/// Trait for credential storage backends
pub trait CredentialStore: Send + Sync {
    /// Store a credential and return its ID
    fn store_credential(
        &mut self,
        name: String,
        description: Option<String>,
        credential: SshCredential,
    ) -> Result<CredentialId>;

    /// Retrieve a credential by ID
    fn get_credential(&self, id: &CredentialId) -> Result<Option<StoredCredential>>;

    /// List all stored credentials (without sensitive data)
    fn list_credentials(&self) -> Result<Vec<CredentialSummary>>;

    /// Update a credential
    #[allow(dead_code)]
    fn update_credential(
        &mut self,
        id: &CredentialId,
        name: String,
        description: Option<String>,
        credential: SshCredential,
    ) -> Result<()>;

    /// Delete a credential
    fn delete_credential(&mut self, id: &CredentialId) -> Result<()>;

    /// Update last used timestamp
    #[allow(dead_code)]
    fn mark_credential_used(&mut self, id: &CredentialId) -> Result<()>;
}

/// Summary of a credential without sensitive data
#[derive(Clone, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: CredentialId,
    pub name: String,
    pub description: Option<String>,
    pub credential_type: String,
    pub username: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<&StoredCredential> for CredentialSummary {
    fn from(stored: &StoredCredential) -> Self {
        let credential_type = match &stored.credential {
            SshCredential::Default => "Default SSH".to_string(),
            SshCredential::Password { .. } => "SSH Password".to_string(),
            SshCredential::Key { .. } => "SSH Key File".to_string(),
            SshCredential::KeyData { .. } => "SSH Key Data".to_string(),
        };

        Self {
            id: stored.id.clone(),
            name: stored.name.clone(),
            description: stored.description.clone(),
            credential_type,
            username: stored.credential.username().map(|u| u.to_string()),
            created_at: stored.created_at,
            last_used: stored.last_used,
        }
    }
}

/// File-based credential store with encryption
pub struct FileCredentialStore {
    storage_path: PathBuf,
    master_password: SensitiveString,
    credentials: HashMap<CredentialId, StoredCredential>,
}

impl FileCredentialStore {
    /// Create a new file-based credential store
    pub fn new(master_password: String) -> Result<Self> {
        let storage_path = Self::get_storage_path()?;
        std::fs::create_dir_all(storage_path.parent().unwrap())?;

        let mut store = Self {
            storage_path,
            master_password: SensitiveString::new(master_password),
            credentials: HashMap::new(),
        };

        // Load existing credentials if the file exists
        if store.storage_path.exists() {
            store.load_credentials()?;
        } else {
            info!("Creating new credential store at {:?}", store.storage_path);
        }

        Ok(store)
    }

    /// Get the default storage path for credentials
    fn get_storage_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "casey", "net-monitor")
            .ok_or_else(|| anyhow!("Could not find project directories"))?;
        Ok(project_dirs.data_dir().join("credentials.enc"))
    }

    /// Load credentials from encrypted file
    fn load_credentials(&mut self) -> Result<()> {
        let encrypted_data = std::fs::read(&self.storage_path)?;
        if encrypted_data.is_empty() {
            return Ok(());
        }

        let decrypted_data = self.decrypt_data(&encrypted_data)?;
        let credentials: HashMap<CredentialId, StoredCredential> =
            serde_json::from_slice(&decrypted_data)?;

        self.credentials = credentials;
        info!("Loaded {} credentials from storage", self.credentials.len());
        Ok(())
    }

    /// Save credentials to encrypted file
    fn save_credentials(&self) -> Result<()> {
        let json_data = serde_json::to_vec(&self.credentials)?;
        let encrypted_data = self.encrypt_data(&json_data)?;
        std::fs::write(&self.storage_path, encrypted_data)?;
        Ok(())
    }

    /// Encrypt data using AES-256-GCM with password-derived key
    fn encrypt_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Derive key from password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(self.master_password.as_str().as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?;

        // Extract first 32 bytes for AES-256 key
        let hash = password_hash.hash.unwrap();
        let key_bytes = hash.as_bytes();
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
        let cipher = Aes256Gcm::new(key);

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut AeadOsRng);

        // Encrypt the data
        let ciphertext = cipher
            .encrypt(&nonce, data)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Prepend salt and nonce to ciphertext
        let mut result = Vec::new();
        result.extend_from_slice(salt.as_str().as_bytes());
        result.push(0); // Separator
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data using AES-256-GCM with password-derived key
    fn decrypt_data(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        // Find separator between salt and nonce+ciphertext
        let separator_pos = encrypted_data
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| anyhow!("Invalid encrypted data format"))?;

        let salt_bytes = &encrypted_data[..separator_pos];
        let salt_str =
            std::str::from_utf8(salt_bytes).map_err(|_| anyhow!("Invalid salt format"))?;
        let salt = SaltString::from_b64(salt_str).map_err(|_| anyhow!("Invalid salt encoding"))?;

        // Derive key from password and salt
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(self.master_password.as_str().as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?;

        let hash = password_hash.hash.unwrap();
        let key_bytes = hash.as_bytes();
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
        let cipher = Aes256Gcm::new(key);

        // Extract nonce and ciphertext
        let nonce_and_ciphertext = &encrypted_data[separator_pos + 1..];
        if nonce_and_ciphertext.len() < 12 {
            return Err(anyhow!("Invalid encrypted data: too short"));
        }

        let nonce = Nonce::from_slice(&nonce_and_ciphertext[..12]);
        let ciphertext = &nonce_and_ciphertext[12..];

        // Decrypt the data
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }

    /// Generate a unique credential ID
    fn generate_credential_id(&self) -> CredentialId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        self.credentials.len().hash(&mut hasher);
        format!("cred_{:016x}", hasher.finish())
    }
}

impl CredentialStore for FileCredentialStore {
    fn store_credential(
        &mut self,
        name: String,
        description: Option<String>,
        credential: SshCredential,
    ) -> Result<CredentialId> {
        let id = self.generate_credential_id();
        let stored_credential = StoredCredential {
            id: id.clone(),
            name,
            description,
            credential,
            created_at: chrono::Utc::now(),
            last_used: None,
        };

        self.credentials.insert(id.clone(), stored_credential);
        self.save_credentials()?;

        info!("Stored credential with ID: {}", id);
        Ok(id)
    }

    fn get_credential(&self, id: &CredentialId) -> Result<Option<StoredCredential>> {
        Ok(self.credentials.get(id).cloned())
    }

    fn list_credentials(&self) -> Result<Vec<CredentialSummary>> {
        let summaries: Vec<CredentialSummary> = self
            .credentials
            .values()
            .map(CredentialSummary::from)
            .collect();
        Ok(summaries)
    }

    fn update_credential(
        &mut self,
        id: &CredentialId,
        name: String,
        description: Option<String>,
        credential: SshCredential,
    ) -> Result<()> {
        if let Some(stored_credential) = self.credentials.get_mut(id) {
            stored_credential.name = name;
            stored_credential.description = description;
            stored_credential.credential = credential;
            self.save_credentials()?;
            info!("Updated credential with ID: {}", id);
            Ok(())
        } else {
            Err(anyhow!("Credential with ID {} not found", id))
        }
    }

    fn delete_credential(&mut self, id: &CredentialId) -> Result<()> {
        if self.credentials.remove(id).is_some() {
            self.save_credentials()?;
            info!("Deleted credential with ID: {}", id);
            Ok(())
        } else {
            Err(anyhow!("Credential with ID {} not found", id))
        }
    }

    fn mark_credential_used(&mut self, id: &CredentialId) -> Result<()> {
        if let Some(stored_credential) = self.credentials.get_mut(id) {
            stored_credential.last_used = Some(chrono::Utc::now());
            self.save_credentials()?;
            Ok(())
        } else {
            Err(anyhow!("Credential with ID {} not found", id))
        }
    }
}

/// SSH key utilities
pub mod ssh_keys {
    use super::*;
    use std::path::Path;

    /// Discover SSH keys in the standard SSH directory
    #[allow(dead_code)]
    pub fn discover_ssh_keys() -> Result<Vec<PathBuf>> {
        // Use HOME environment variable if set (for testing), otherwise use dirs::home_dir()
        let home_dir = std::env::var("HOME")
            .ok()
            .and_then(|p| Some(PathBuf::from(p)))
            .or_else(|| dirs::home_dir())
            .ok_or_else(|| anyhow!("Could not find home directory"))?;

        let ssh_dir = home_dir.join(".ssh");

        if !ssh_dir.exists() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        for entry in std::fs::read_dir(&ssh_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && is_private_key(&path)? {
                keys.push(path);
            }
        }

        Ok(keys)
    }

    /// Check if a file is likely a private SSH key
    #[allow(dead_code)]
    fn is_private_key(path: &Path) -> Result<bool> {
        let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");

        // Skip known public key files
        if filename.ends_with(".pub") || filename.ends_with(".certificate") {
            return Ok(false);
        }

        // Check common private key names
        if ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"].contains(&filename) {
            return Ok(true);
        }

        // Check file content for private key headers
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let content = content.trim();
                Ok(content.starts_with("-----BEGIN")
                    && (content.contains("PRIVATE KEY") || content.contains("OPENSSH PRIVATE KEY")))
            }
            Err(_) => Ok(false),
        }
    }

    /// Validate an SSH private key format
    #[allow(dead_code)]
    pub fn validate_private_key(key_data: &str) -> Result<()> {
        let key_data = key_data.trim();

        if key_data.starts_with("-----BEGIN") && key_data.contains("PRIVATE KEY") {
            // PEM format key
            Ok(())
        } else if key_data.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----") {
            // OpenSSH format key
            Ok(())
        } else {
            Err(anyhow!("Invalid private key format"))
        }
    }
}
