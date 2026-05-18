use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StickyEntry {
    pub id: i64,
    pub content: String,
    pub content_type: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub always_on_top: bool,
    pub created_at: i64,
}
