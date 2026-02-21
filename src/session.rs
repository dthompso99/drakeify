use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};

use crate::llm::OllamaMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<OllamaMessage>,
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub title: Option<String>,
    pub tags: Vec<String>,
}

pub struct SessionManager {
    sessions_dir: PathBuf,
    current_session: Option<Session>,
    auto_save: bool,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(sessions_dir: impl AsRef<Path>, auto_save: bool) -> Result<Self> {
        let sessions_dir = sessions_dir.as_ref().to_path_buf();
        
        // Create sessions directory if it doesn't exist
        if !sessions_dir.exists() {
            fs::create_dir_all(&sessions_dir)?;
        }
        
        Ok(Self {
            sessions_dir,
            current_session: None,
            auto_save,
        })
    }
    
    /// Create a new session
    pub fn new_session(&mut self) -> Result<String> {
        let session_id = Self::generate_session_id();
        let now = Utc::now();
        
        let session = Session {
            id: session_id.clone(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            metadata: SessionMetadata {
                title: None,
                tags: Vec::new(),
            },
        };
        
        self.current_session = Some(session);
        Ok(session_id)
    }
    
    /// Load an existing session
    pub fn load_session(&mut self, session_id: &str) -> Result<()> {
        let session_path = self.sessions_dir.join(format!("{}.json", session_id));
        
        if !session_path.exists() {
            anyhow::bail!("Session not found: {}", session_id);
        }
        
        let session_json = fs::read_to_string(&session_path)?;
        let session: Session = serde_json::from_str(&session_json)?;
        
        self.current_session = Some(session);
        Ok(())
    }
    
    /// Save the current session
    pub fn save_session(&self) -> Result<()> {
        if let Some(session) = &self.current_session {
            let session_path = self.sessions_dir.join(format!("{}.json", session.id));
            let session_json = serde_json::to_string_pretty(session)?;
            fs::write(&session_path, session_json)?;
        }
        Ok(())
    }
    
    /// Add a message to the current session
    pub fn add_message(&mut self, message: OllamaMessage) -> Result<()> {
        if let Some(session) = &mut self.current_session {
            session.messages.push(message);
            session.updated_at = Utc::now();
            
            if self.auto_save {
                self.save_session()?;
            }
        }
        Ok(())
    }
    
    /// Get messages from the current session
    pub fn get_messages(&self) -> Vec<OllamaMessage> {
        self.current_session
            .as_ref()
            .map(|s| s.messages.clone())
            .unwrap_or_default()
    }

    /// Update all messages in the current session (replaces existing messages)
    pub fn update_messages(&mut self, messages: Vec<OllamaMessage>) -> Result<()> {
        if let Some(session) = &mut self.current_session {
            session.messages = messages;
            session.updated_at = Utc::now();

            if self.auto_save {
                self.save_session()?;
            }
        }
        Ok(())
    }

    /// Get current session ID
    pub fn get_session_id(&self) -> Option<String> {
        self.current_session.as_ref().map(|s| s.id.clone())
    }
    
    /// List all available sessions
    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();
        
        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(stem.to_string());
                }
            }
        }
        
        Ok(sessions)
    }
    
    /// Generate a unique session ID
    fn generate_session_id() -> String {
        format!("session_{}", Utc::now().timestamp())
    }
}

