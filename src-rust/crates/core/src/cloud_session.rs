//! Cloud session API — mirrors src/remote/sdkMessageAdapter.ts.
//!
//! Converts between internal Message types and the cloud API format.
//! Provides CRUD operations for cloud-hosted sessions.

use serde::{Deserialize, Serialize};
use crate::types::{Message, Role, MessageContent};

// ---------------------------------------------------------------------------
// Cloud session API types
// ---------------------------------------------------------------------------

/// Options for creating a new cloud session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSessionCreateOpts {
    pub project_root: Option<String>,
    pub model: String,
    pub title: Option<String>,
}

/// A cloud session detail (with full message list).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSessionDetail {
    pub id: String,
    pub title: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<CloudMessage>,
}

/// A message in the cloud API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudMessage {
    pub id: String,
    pub role: String,    // "user" | "assistant"
    pub content: String, // Plain text for now
    pub created_at: u64,
    pub session_id: String,
}

// ---------------------------------------------------------------------------
// SDK message adapter
// ---------------------------------------------------------------------------

/// Convert an internal `Message` to a `CloudMessage`.
pub fn message_to_cloud(msg: &Message, session_id: &str, msg_id: &str, ts: u64) -> CloudMessage {
    let role = match msg.role {
        Role::User => "user".to_string(),
        Role::Assistant => "assistant".to_string(),
    };
    let content = match &msg.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => {
            blocks.iter().filter_map(|b| {
                if let crate::types::ContentBlock::Text { text } = b {
                    Some(text.as_str())
                } else {
                    None
                }
            }).collect::<Vec<_>>().join("\n")
        }
    };
    CloudMessage {
        id: msg_id.to_string(),
        role,
        content,
        created_at: ts,
        session_id: session_id.to_string(),
    }
}

/// Convert a `CloudMessage` back to an internal `Message`.
pub fn cloud_to_message(cloud: &CloudMessage) -> Message {
    let role = if cloud.role == "assistant" { Role::Assistant } else { Role::User };
    Message {
        role,
        content: MessageContent::Text(cloud.content.clone()),
        uuid: None,
        cost: None,
    }
}

// ---------------------------------------------------------------------------
// Cloud session API client
// ---------------------------------------------------------------------------

/// Thin client for the cloud session REST API.
pub struct CloudSessionClient {
    base_url: String,
    access_token: String,
    http: reqwest::Client,
}

impl CloudSessionClient {
    pub fn new(access_token: String) -> Self {
        Self {
            base_url: "https://api.claude.ai".to_string(),
            access_token,
            http: reqwest::Client::new(),
        }
    }

    /// List all cloud sessions.
    pub async fn list(&self) -> Result<Vec<crate::remote_session::CloudSession>, String> {
        let resp = self.http
            .get(format!("{}/api/sessions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send().await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    }

    /// Fetch full session details including messages.
    pub async fn fetch(&self, session_id: &str) -> Result<CloudSessionDetail, String> {
        let resp = self.http
            .get(format!("{}/api/sessions/{}", self.base_url, session_id))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send().await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    }

    /// Push new messages to a cloud session.
    pub async fn push_messages(
        &self,
        session_id: &str,
        messages: &[CloudMessage],
    ) -> Result<(), String> {
        let resp = self.http
            .post(format!("{}/api/sessions/{}/messages", self.base_url, session_id))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(messages)
            .send().await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        Ok(())
    }

    /// Create a new cloud session.
    pub async fn create(&self, opts: CloudSessionCreateOpts) -> Result<crate::remote_session::CloudSession, String> {
        let resp = self.http
            .post(format!("{}/api/sessions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&opts)
            .send().await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    }

    /// Delete a cloud session.
    pub async fn delete(&self, session_id: &str) -> Result<(), String> {
        let resp = self.http
            .delete(format!("{}/api/sessions/{}", self.base_url, session_id))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send().await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        Ok(())
    }
}
