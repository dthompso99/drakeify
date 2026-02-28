use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate::llm::OllamaMessage;
use crate::database::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<OllamaMessage>,
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionMetadata {
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Capture any additional fields (like _webhook_context) that plugins might add
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub struct SessionManager {
    database: Arc<Database>,
    account_id: String,
    current_session: Option<Session>,
    auto_save: bool,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(database: Arc<Database>, account_id: String, auto_save: bool) -> Result<Self> {
        Ok(Self {
            database,
            account_id,
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
                extra: std::collections::HashMap::new(),
            },
        };
        
        self.current_session = Some(session);
        Ok(session_id)
    }
    
    /// Load an existing session
    pub async fn load_session(&mut self, session_id: &str) -> Result<()> {
        let result = self.database.get_session(session_id, &self.account_id).await?;

        if let Some((messages_json, metadata_json)) = result {
            let messages: Vec<OllamaMessage> = serde_json::from_str(&messages_json)?;
            let metadata: SessionMetadata = serde_json::from_str(&metadata_json)?;

            let session = Session {
                id: session_id.to_string(),
                created_at: Utc::now(), // We'll get this from DB in future if needed
                updated_at: Utc::now(),
                messages,
                metadata,
            };

            self.current_session = Some(session);
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", session_id);
        }
    }
    
    /// Save the current session
    pub async fn save_session(&self) -> Result<()> {
        if let Some(session) = &self.current_session {
            let messages_json = serde_json::to_string(&session.messages)?;
            let metadata_json = serde_json::to_string(&session.metadata)?;

            self.database.upsert_session(
                &session.id,
                &self.account_id,
                &messages_json,
                &metadata_json,
            ).await?;
        }
        Ok(())
    }
    
    /// Add a message to the current session
    pub async fn add_message(&mut self, message: OllamaMessage) -> Result<()> {
        if let Some(session) = &mut self.current_session {
            session.messages.push(message);
            session.updated_at = Utc::now();

            if self.auto_save {
                self.save_session().await?;
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
    pub async fn update_messages(&mut self, messages: Vec<OllamaMessage>) -> Result<()> {
        if let Some(session) = &mut self.current_session {
            session.messages = messages;
            session.updated_at = Utc::now();

            if self.auto_save {
                self.save_session().await?;
            }
        }
        Ok(())
    }

    /// Get current session ID
    pub fn get_session_id(&self) -> Option<String> {
        self.current_session.as_ref().map(|s| s.id.clone())
    }
    
    /// List all available sessions for the current account
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        self.database.list_sessions(&self.account_id).await
    }
    
    /// Generate a unique session ID
    fn generate_session_id() -> String {
        format!("session_{}", Utc::now().timestamp())
    }
}

