use crate::domain::ClipboardEntry;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub trait TagRepository {
    fn add_tag(&self, name: &str) -> rusqlite::Result<()>;
    fn remove_tag(&self, name: &str) -> rusqlite::Result<()>;
    fn set_tag_color(&self, name: &str, color: &str) -> rusqlite::Result<()>;
    fn get_all_tags(&self) -> rusqlite::Result<Vec<TagEntry>>;
    fn get_tag_color(&self, name: &str) -> rusqlite::Result<Option<String>>;
    fn add_entry_tag(&self, entry_id: i64, tag: &str) -> rusqlite::Result<()>;
    fn remove_entry_tag(&self, entry_id: i64, tag: &str) -> rusqlite::Result<()>;
    fn get_entry_tags(&self, entry_id: i64) -> rusqlite::Result<Vec<String>>;
    fn rename_tag(&self, old_name: &str, new_name: &str) -> rusqlite::Result<()>;
}

#[derive(Debug, Clone)]
pub struct TagEntry {
    pub name: String,
    pub color: Option<String>,
}

pub struct SqliteTagRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteTagRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Get all tags with their entry count, sorted by count descending.
    pub fn get_all_tags_with_count(&self) -> rusqlite::Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT tag, COUNT(*) as cnt FROM entry_tags GROUP BY tag ORDER BY cnt DESC"
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Get all clipboard entries that have a specific tag.
    pub fn get_entries_by_tag(&self, tag: &str) -> rusqlite::Result<Vec<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT ch.id, ch.content_type, ch.content, ch.html_content, ch.source_app,
                    ch.timestamp, ch.preview, ch.is_pinned, ch.tags, ch.use_count,
                    ch.is_external, ch.pinned_order, ch.source_app_path
             FROM clipboard_history ch
             INNER JOIN entry_tags et ON ch.id = et.entry_id
             WHERE et.tag = ?1
             ORDER BY ch.timestamp DESC"
        )?;
        let entries = stmt
            .query_map(rusqlite::params![tag], |row| {
                let tags_str: String = row.get::<_, String>(8).unwrap_or_else(|_| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                Ok(ClipboardEntry {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content: row.get(2)?,
                    html_content: row.get(3)?,
                    source_app: row.get(4)?,
                    timestamp: row.get(5)?,
                    preview: row.get(6)?,
                    is_pinned: row.get::<_, i32>(7)? == 1,
                    tags,
                    use_count: row.get(9).unwrap_or(0),
                    is_external: row.get::<_, i32>(10)? == 1,
                    pinned_order: row.get(11).unwrap_or(0),
                    source_app_path: row.get(12).unwrap_or(None),
                    file_preview_exists: true,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }
}

impl TagRepository for SqliteTagRepository {
    fn add_tag(&self, name: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO saved_tags (name) VALUES (?1)",
            rusqlite::params![name],
        )?;
        Ok(())
    }

    fn remove_tag(&self, name: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM saved_tags WHERE name = ?1",
            rusqlite::params![name],
        )?;
        Ok(())
    }

    fn set_tag_color(&self, name: &str, color: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO saved_tags (name, color) VALUES (?1, ?2)",
            rusqlite::params![name, color],
        )?;
        Ok(())
    }

    fn get_all_tags(&self) -> rusqlite::Result<Vec<TagEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name, color FROM saved_tags")?;
        let entries = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let color: Option<String> = row.get(1)?;
                Ok(TagEntry { name, color })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    fn get_tag_color(&self, name: &str) -> rusqlite::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT color FROM saved_tags WHERE name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        );
        match result {
            Ok(color) => Ok(color),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn add_entry_tag(&self, entry_id: i64, tag: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?1, ?2)",
            rusqlite::params![entry_id, tag],
        )?;
        Ok(())
    }

    fn remove_entry_tag(&self, entry_id: i64, tag: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM entry_tags WHERE entry_id = ?1 AND tag = ?2",
            rusqlite::params![entry_id, tag],
        )?;
        Ok(())
    }

    fn get_entry_tags(&self, entry_id: i64) -> rusqlite::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT tag FROM entry_tags WHERE entry_id = ?1")?;
        let tags = stmt
            .query_map(rusqlite::params![entry_id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tags)
    }

    fn rename_tag(&self, old_name: &str, new_name: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO saved_tags (name, color) SELECT ?1, color FROM saved_tags WHERE name = ?2",
            rusqlite::params![new_name, old_name],
        )?;
        conn.execute(
            "UPDATE entry_tags SET tag = ?1 WHERE tag = ?2",
            rusqlite::params![new_name, old_name],
        )?;
        conn.execute(
            "DELETE FROM saved_tags WHERE name = ?1",
            rusqlite::params![old_name],
        )?;
        Ok(())
    }
}
