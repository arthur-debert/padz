use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    Project,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    // We store the title in metadata to list without reading content files
    pub title: String,
}

impl Metadata {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            is_pinned: false,
            pinned_at: None,
            is_deleted: false,
            deleted_at: None,
            title,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pad {
    pub metadata: Metadata,
    pub content: String,
}

impl Pad {
    pub fn new(title: String, content: String) -> Self {
        Self {
            metadata: Metadata::new(title),
            content,
        }
    }
}
