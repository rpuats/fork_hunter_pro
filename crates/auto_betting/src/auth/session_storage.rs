//! Session storage - encrypted persistence for browser sessions
//! Uses machine-specific key for encryption

use super::SessionCookies;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;

const SESSION_DIR: &str = "data/sessions";

/// Session storage with encryption
pub struct SessionStorage {
    session_dir: PathBuf,
    encryption_key: Vec<u8>,
}

impl SessionStorage {
    pub fn new() -> Self {
        let session_dir = PathBuf::from(SESSION_DIR);
        let encryption_key = Self::derive_machine_key();
        
        // Ensure directory exists
        if !session_dir.exists() {
            let _ = std::fs::create_dir_all(&session_dir);
        }
        
        Self {
            session_dir,
            encryption_key,
        }
    }

    /// Save session to disk with encryption
    pub async fn save_session(&self, bookmaker_id: &str, session: &SessionCookies) -> Result<()> {
        let path = self.session_file_path(bookmaker_id);
        
        // Serialize
        let json = serde_json::to_string(session)
            .context("Failed to serialize session")?;
        
        // Encrypt
        let encrypted = self.encrypt(&json)?;
        
        // Write to file
        tokio::fs::write(&path, encrypted)
            .await
            .context("Failed to write session file")?;
        
        tracing::info!("Saved session for {}", bookmaker_id);
        Ok(())
    }

    /// Load session from disk
    pub async fn load_session(&self, bookmaker_id: &str) -> Result<Option<SessionCookies>> {
        let path = self.session_file_path(bookmaker_id);
        
        if !path.exists() {
            return Ok(None);
        }
        
        // Read file
        let encrypted = tokio::fs::read(&path)
            .await
            .context("Failed to read session file")?;
        
        // Decrypt
        let json = self.decrypt(&encrypted)?;
        
        // Deserialize
        let session: SessionCookies = serde_json::from_str(&json)
            .context("Failed to deserialize session")?;
        
        // Check if session is expired (30 days)
        let age = Utc::now() - session.created_at;
        if age.num_days() > 30 {
            tracing::warn!("Session for {} is expired ({} days old)", bookmaker_id, age.num_days());
            let _ = tokio::fs::remove_file(&path).await;
            return Ok(None);
        }
        
        tracing::info!("Loaded session for {}", bookmaker_id);
        Ok(Some(session))
    }

    /// Delete session
    pub async fn delete_session(&self, bookmaker_id: &str) -> Result<()> {
        let path = self.session_file_path(bookmaker_id);
        if path.exists() {
            tokio::fs::remove_file(&path)
                .await
                .context("Failed to delete session file")?;
            tracing::info!("Deleted session for {}", bookmaker_id);
        }
        Ok(())
    }

    /// List all saved sessions
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();
        
        let entries = tokio::fs::read_dir(&self.session_dir)
            .await
            .context("Failed to read session directory")?;
        
        // Need to use next_entry in a loop for async
        use tokio::fs::ReadDir;
        
        // Simpler approach - use std::fs for listing
        let entries = std::fs::read_dir(&self.session_dir)?;
        for entry in entries {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with("_session.enc") {
                    let bk_id = name.replace("_session.enc", "");
                    sessions.push(bk_id);
                }
            }
        }
        
        Ok(sessions)
    }

    fn session_file_path(&self, bookmaker_id: &str) -> PathBuf {
        self.session_dir.join(format!("{}_session.enc", bookmaker_id))
    }

    /// Derive encryption key from machine-specific data
    /// This ensures sessions can only be decrypted on the same machine
    fn derive_machine_key() -> Vec<u8> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // Combine machine-specific data
        let machine_data = format!(
            "{}{}{}",
            std::env::consts::OS,
            whoami::username(),
            option_env!("COMPUTERNAME").unwrap_or("unknown")
        );
        
        let mut hasher = DefaultHasher::new();
        machine_data.hash(&mut hasher);
        let hash1 = hasher.finish();
        
        // Create 32-byte key
        let mut key = Vec::with_capacity(32);
        for i in 0..4 {
            let byte = ((hash1 >> (i * 8)) & 0xFF) as u8;
            key.push(byte);
        }
        
        // Pad to 32 bytes
        while key.len() < 32 {
            key.push(0);
        }
        
        key
    }

    /// Simple XOR encryption (for demo - use proper crypto in production)
    fn encrypt(&self, data: &str) -> Result<Vec<u8>> {
        let data_bytes = data.as_bytes();
        let mut encrypted = Vec::with_capacity(data_bytes.len() + 16);
        
        // Add a simple header
        encrypted.extend_from_slice(b"SESSv1");
        
        // XOR with key
        for (i, byte) in data_bytes.iter().enumerate() {
            let key_byte = self.encryption_key[i % self.encryption_key.len()];
            encrypted.push(byte ^ key_byte);
        }
        
        Ok(encrypted)
    }

    /// Decrypt data
    fn decrypt(&self, data: &[u8]) -> Result<String> {
        // Check header
        if data.len() < 6 || &data[0..6] != b"SESSv1" {
            return Err(anyhow::anyhow!("Invalid session file format"));
        }
        
        let encrypted = &data[6..];
        let mut decrypted = Vec::with_capacity(encrypted.len());
        
        // XOR with key
        for (i, byte) in encrypted.iter().enumerate() {
            let key_byte = self.encryption_key[i % self.encryption_key.len()];
            decrypted.push(byte ^ key_byte);
        }
        
        String::from_utf8(decrypted).context("Invalid UTF-8 in decrypted data")
    }
}

impl Default for SessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// For production, replace with proper encryption (e.g., AES-GCM via ring crate)
/// This is a simplified implementation for MVP
#[cfg(feature = "strong-encryption")]
mod strong_crypto {
    use anyhow::Result;
    
    pub fn encrypt(data: &str, key: &[u8]) -> Result<Vec<u8>> {
        // Use ring::aead::AES_256_GCM or similar
        todo!("Implement strong encryption with ring crate")
    }
    
    pub fn decrypt(data: &[u8], key: &[u8]) -> Result<String> {
        todo!("Implement strong decryption with ring crate")
    }
}
