use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

use crate::database::{
    calc_image_hash, calc_text_hash, encryption, is_text_type,
    save_image_to_file, ENCRYPT_PREFIX,
};
use crate::domain::ClipboardEntry;
use crate::infrastructure::repository::settings_repo::{
    SettingsRepository, SqliteSettingsRepository,
};

pub trait ClipboardRepository {
    fn save(
        &self,
        entry: &ClipboardEntry,
        data_dir: Option<&std::path::Path>,
    ) -> Result<i64, String>;
    fn get_history(
        &self,
        limit: i32,
        offset: i32,
        content_type: Option<&str>,
    ) -> Result<Vec<ClipboardEntry>, String>;
    fn search(&self, query: &str, limit: i32) -> Result<Vec<ClipboardEntry>, String>;
    fn delete(&self, id: i64, data_dir: Option<&std::path::Path>) -> Result<(), String>;
    fn clear(&self, data_dir: Option<&std::path::Path>) -> Result<(), String>;
    fn get_count(&self) -> Result<i64, String>;
    fn increment_use_count(&self, id: i64) -> Result<(), String>;
    fn touch_entry(&self, id: i64, timestamp: i64) -> Result<(), String>;
    fn toggle_pin(&self, id: i64, is_pinned: bool) -> Result<(), String>;
    fn update_pinned_order(&self, orders: Vec<(i64, i64)>) -> Result<(), String>;
    fn get_entry_by_id(&self, id: i64) -> Result<Option<ClipboardEntry>, String>;
    fn get_entry_by_content(
        &self,
        content: &str,
        content_type: Option<&str>,
    ) -> Result<Option<i64>, String>;
    fn update_entry_content(&self, id: i64, content: &str, preview: &str) -> Result<(), String>;
    fn get_entry_content(&self, id: i64) -> Result<Option<String>, String>;
    fn get_entry_content_full(&self, id: i64) -> Result<Option<(String, String)>, String>;
    fn get_entry_content_with_html(
        &self,
        id: i64,
    ) -> Result<Option<(String, String, Option<String>)>, String>;
}

pub struct SqliteClipboardRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteClipboardRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    fn maybe_encrypt_text(&self, text: &str) -> String {
        if let Some(encrypted) = encryption::encrypt_value(text) {
            encrypted
        } else {
            text.to_string()
        }
    }

    fn maybe_decrypt_text(&self, value: &str) -> String {
        if value.starts_with(ENCRYPT_PREFIX) {
            encryption::decrypt_value(value).unwrap_or_else(|| value.to_string())
        } else {
            value.to_string()
        }
    }

    fn map_row(&self, row: &rusqlite::Row) -> rusqlite::Result<ClipboardEntry> {
        let tags_str: String = row.get::<_, String>(8).unwrap_or_else(|_| "[]".to_string());
        let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
        let content_raw: String = row.get(2)?;
        let html_raw: Option<String> = row.get(3).ok();
        let preview_raw: String = row.get(6)?;

        let content = self.maybe_decrypt_text(&content_raw);
        let preview = self.maybe_decrypt_text(&preview_raw);
        let html_content = html_raw.map(|v| self.maybe_decrypt_text(&v));

        Ok(ClipboardEntry {
            id: row.get(0)?,
            content_type: row.get(1)?,
            content,
            html_content,
            source_app: row.get(4)?,
            timestamp: row.get(5)?,
            preview,
            is_pinned: row.get::<_, i32>(7)? == 1,
            tags,
            use_count: row.get(9).unwrap_or(0),
            is_external: row.get::<_, i32>(10)? == 1,
            pinned_order: row.get(11).unwrap_or(0),
            source_app_path: row.get(12).unwrap_or(None),
            file_preview_exists: true,
        })
    }

    fn save_with_conn(
        &self,
        conn: &Connection,
        entry: &ClipboardEntry,
        data_dir: Option<&std::path::Path>,
    ) -> Result<i64, String> {
        let (content, is_external, display_content, display_preview) =
            if entry.content_type == "image" && entry.content.starts_with("data:image/") {
                if let Some(ref dir) = data_dir {
                    if let Some(file_path) = save_image_to_file(&entry.content, dir) {
                        let encrypted_path = self.maybe_encrypt_text(&file_path);
                        let preview = entry.content.chars().take(100).collect::<String>();
                        (
                            encrypted_path,
                            true,
                            format!("[image] {}", file_path),
                            preview,
                        )
                    } else {
                        let encrypted = self.maybe_encrypt_text(&entry.content);
                        let preview = entry.content.chars().take(100).collect::<String>();
                        (encrypted, entry.is_external, entry.content.clone(), preview)
                    }
                } else {
                    let encrypted = self.maybe_encrypt_text(&entry.content);
                    let preview = entry.content.chars().take(100).collect::<String>();
                    (encrypted, entry.is_external, entry.content.clone(), preview)
                }
            } else if entry.content_type == "image" && !entry.content.starts_with("data:image/") {
                let display = if entry.is_external {
                    if entry.content.contains('%') {
                        let _ = urlencoding::decode(&entry.content);
                        format!("[image] {}", entry.content.clone())
                    } else {
                        format!("[image] {}", entry.content.clone())
                    }
                } else {
                    format!("[image] {}", entry.content.clone())
                };
                let encrypted = self.maybe_encrypt_text(&entry.content);
                (
                    encrypted,
                    entry.is_external,
                    display,
                    entry.content.chars().take(100).collect::<String>(),
                )
            } else if entry.content_type == "file" || entry.content_type == "video" {
                let encrypted = self.maybe_encrypt_text(&entry.content);
                let display = if entry.is_external {
                    format!("[{}] {}", entry.content_type, entry.content)
                } else {
                    entry.content.clone()
                };
                (
                    encrypted,
                    entry.is_external,
                    display,
                    entry.content.chars().take(100).collect::<String>(),
                )
            } else {
                // text-based content
                let encrypted = self.maybe_encrypt_text(&entry.content);
                let preview_encrypted = self.maybe_encrypt_text(&entry.preview);
                (
                    encrypted,
                    entry.is_external,
                    entry.content.clone(),
                    preview_encrypted,
                )
            };

        // Calculate hash
        let content_hash: i64 = if is_text_type(&entry.content_type) {
            calc_text_hash(&display_content) as i64
        } else if entry.content_type == "image" {
            if entry.content.starts_with("data:image/") {
                calc_image_hash(&entry.content).unwrap_or(0)
            } else {
                // file-based image: use a hash of the file path
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                entry.content.hash(&mut hasher);
                hasher.finish() as i64
            }
        } else {
            0
        };

        // Check for duplicate
        if entry.content_type != "image" && content_hash != 0 {
            let settings_repo = SqliteSettingsRepository::new(self.conn.clone());
            let dedup_enabled = settings_repo
                .get("app.deduplicate")
                .unwrap_or(Some("true".to_string()))
                .unwrap_or_else(|| "true".to_string());
            if dedup_enabled == "true" {
                let existing: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM clipboard_history WHERE content_hash = ?1 AND content_type = ?2 ORDER BY timestamp DESC LIMIT 1",
                        params![content_hash, entry.content_type],
                        |row| row.get(0),
                    )
                    .unwrap_or(None);
                if let Some(existing_id) = existing {
                    // Update existing entry with new timestamp but keep content
                    let _ = conn.execute(
                        "UPDATE clipboard_history SET timestamp = ?1, use_count = use_count + 1 WHERE id = ?2",
                        params![entry.timestamp, existing_id],
                    );
                    return Ok(existing_id);
                }
            }
        }

        // Determine tags JSON
        let tags_json = serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".to_string());

        // Insert entry
        conn.execute(
            "INSERT INTO clipboard_history (content_type, content, html_content, source_app, source_app_path, timestamp, preview, is_pinned, content_hash, tags, use_count, is_external, pinned_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                entry.content_type,
                content,
                entry.html_content,
                entry.source_app,
                entry.source_app_path,
                entry.timestamp,
                display_preview,
                entry.is_pinned as i32,
                content_hash,
                tags_json,
                entry.use_count,
                is_external as i32,
                entry.pinned_order,
            ],
        ).map_err(|e| e.to_string())?;

        let new_id = conn.last_insert_rowid();

        // Insert tags
        for tag in &entry.tags {
            let t = tag.trim().to_string();
            if !t.is_empty() {
                conn.execute(
                    "INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?1, ?2)",
                    params![new_id, t],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        // Enforce limit
        enforce_limit_with_conn(self.conn.clone(), data_dir).ok();

        Ok(new_id)
    }
}

fn enforce_limit_with_conn(
    conn: Arc<Mutex<Connection>>,
    data_dir: Option<&std::path::Path>,
) -> Result<(), String> {
    let settings_repo = SqliteSettingsRepository::new(conn.clone());
    let limit_enabled = settings_repo
        .get("app.persistent_limit_enabled")
        .unwrap_or(Some("true".to_string()))
        .unwrap_or_else(|| "true".to_string());
    if limit_enabled != "true" {
        return Ok(());
    }
    let limit_str = settings_repo
        .get("app.persistent_limit")
        .unwrap_or(Some("500".to_string()))
        .unwrap_or_else(|| "500".to_string());
    let limit: i64 = limit_str.parse().unwrap_or(500);

    let conn = conn.lock().map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM clipboard_history", [], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    if count > limit.max(100) {
        let excess = count - limit;
        let mut stmt = conn
            .prepare(
                "SELECT id, content, is_external FROM clipboard_history
                 WHERE is_pinned = 0
                 ORDER BY timestamp ASC
                 LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(i64, String, bool)> = stmt
            .query_map(params![excess], |row| {
                let is_ext: i32 = row.get(2)?;
                Ok((row.get(0)?, row.get(1)?, is_ext != 0))
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();

        for (id, content_raw, is_external) in &rows {
            if *is_external {
                if let Some(dir) = data_dir {
                    let path = std::path::Path::new(&content_raw);
                    let attachments_dir = dir.join("attachments");
                    if path.starts_with(&attachments_dir) && path.exists() {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
            let _ = conn.execute("DELETE FROM entry_tags WHERE entry_id = ?1", params![id]);
            let _ = conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id]);
        }
    }
    Ok(())
}

impl ClipboardRepository for SqliteClipboardRepository {
    fn save(
        &self,
        entry: &ClipboardEntry,
        data_dir: Option<&std::path::Path>,
    ) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        self.save_with_conn(&conn, entry, data_dir)
    }

    fn get_history(
        &self,
        limit: i32,
        offset: i32,
        content_type: Option<&str>,
    ) -> Result<Vec<ClipboardEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let (sql, type_filter): (&str, bool) = if let Some(ct) = content_type {
            if ct == "image" {
                ("WHERE content_type IN ('image')", true)
            } else if ct == "text" {
                (
                    "WHERE content_type IN ('text','code','url','rich_text')",
                    true,
                )
            } else {
                ("WHERE content_type = ?3", true)
            }
        } else {
            ("", false)
        };

        let query = format!(
            "SELECT id, content_type, content, html_content, source_app,
                    timestamp, preview, is_pinned, tags, use_count, is_external,
                    pinned_order, source_app_path
             FROM clipboard_history {}
             ORDER BY is_pinned DESC, pinned_order DESC, timestamp DESC
             LIMIT ?1 OFFSET ?2",
            sql
        );

        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

        let rows: Vec<ClipboardEntry> = if type_filter {
            if let Some(ct) = content_type {
                let clean_ct = if ct == "image" { "" } else { ct };
                stmt.query_map(params![limit, offset, clean_ct], |row| self.map_row(row))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect()
            } else {
                stmt.query_map(params![limit, offset], |row| self.map_row(row))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect()
            }
        } else {
            stmt.query_map(params![limit, offset], |row| self.map_row(row))
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect()
        };

        Ok(rows)
    }

    fn search(&self, query_text: &str, limit: i32) -> Result<Vec<ClipboardEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Phase 1: SQL LIKE search on content, preview, source_app — but EXCLUDE encrypted rows
        let like_pattern = format!("%{}%", query_text);
        let sql_search = format!(
            "SELECT id, content_type, content, html_content, source_app,
                    timestamp, preview, is_pinned, tags, use_count, is_external,
                    pinned_order, source_app_path
             FROM clipboard_history
             WHERE (content LIKE ?1 OR preview LIKE ?1 OR source_app LIKE ?1 OR source_app_path LIKE ?1
                    OR tags LIKE ?1)
               AND content NOT LIKE '{}%'
             ORDER BY is_pinned DESC, pinned_order DESC, timestamp DESC
             LIMIT ?2",
            ENCRYPT_PREFIX.replace("%", "%%")
        );

        let mut stmt = conn.prepare(&sql_search).map_err(|e| e.to_string())?;
        let mut results: Vec<ClipboardEntry> = stmt
            .query_map(params![like_pattern, limit], |row| self.map_row(row))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();

        // Phase 2: if results are few, also scan decrypted content of sensitive entries
        if results.len() < (limit as usize) {
            let sql_sensitive = format!(
                "SELECT id, content_type, content, html_content, source_app,
                        timestamp, preview, is_pinned, tags, use_count, is_external,
                        pinned_order, source_app_path
                 FROM clipboard_history
                 WHERE content LIKE '{}%'
                 ORDER BY is_pinned DESC, pinned_order DESC, timestamp DESC",
                ENCRYPT_PREFIX.replace("%", "%%")
            );
            let mut stmt_sensitive = conn.prepare(&sql_sensitive).map_err(|e| e.to_string())?;
            let sensitive_rows: Vec<ClipboardEntry> = stmt_sensitive
                .query_map([], |row| self.map_row(row))
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();

            let query_lower = query_text.to_lowercase();
            for entry in sensitive_rows {
                if results.len() >= (limit as usize) {
                    break;
                }
                let content_lower = entry.content.to_lowercase();
                let preview_lower = entry.preview.to_lowercase();
                if content_lower.contains(&query_lower)
                    || preview_lower.contains(&query_lower)
                    || entry.source_app.to_lowercase().contains(&query_lower)
                {
                    results.push(entry);
                }
            }
        }

        Ok(results)
    }

    fn delete(&self, id: i64, data_dir: Option<&std::path::Path>) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        if let Some(dir) = data_dir {
            let attachments_dir = dir.join("attachments");
            let result: Result<(String, bool), rusqlite::Error> = conn.query_row(
                "SELECT content, is_external FROM clipboard_history WHERE id = ?1",
                params![id],
                |row| {
                    let content: String = row.get(0)?;
                    let is_ext: i32 = row.get(1)?;
                    Ok((content, is_ext != 0))
                },
            );
            if let Ok((content_raw, is_external)) = result {
                if is_external {
                    let content_path = self.maybe_decrypt_text(&content_raw);
                    let path = std::path::Path::new(&content_path);
                    if path.starts_with(&attachments_dir) && path.exists() {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }

        let _ = conn.execute("DELETE FROM entry_tags WHERE entry_id = ?1", params![id]);
        conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn clear(&self, data_dir: Option<&std::path::Path>) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        if let Some(dir) = data_dir {
            let attachments_dir = dir.join("attachments");
            if attachments_dir.exists() {
                let _ = std::fs::remove_dir_all(&attachments_dir);
            }
        }

        let _ = conn.execute("DELETE FROM entry_tags", []);
        conn.execute("DELETE FROM clipboard_history", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get_count(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_history", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;
        Ok(count)
    }

    fn increment_use_count(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_history SET use_count = use_count + 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn touch_entry(&self, id: i64, timestamp: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_history SET timestamp = ?1 WHERE id = ?2",
            params![timestamp, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn toggle_pin(&self, id: i64, is_pinned: bool) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_history SET is_pinned = ?1 WHERE id = ?2",
            params![is_pinned as i32, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn update_pinned_order(&self, orders: Vec<(i64, i64)>) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        for (id, order) in &orders {
            conn.execute(
                "UPDATE clipboard_history SET pinned_order = ?1 WHERE id = ?2",
                params![order, id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn get_entry_by_id(&self, id: i64) -> Result<Option<ClipboardEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT id, content_type, content, html_content, source_app,
                    timestamp, preview, is_pinned, tags, use_count, is_external,
                    pinned_order, source_app_path
             FROM clipboard_history WHERE id = ?1",
            params![id],
            |row| self.map_row(row),
        );
        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn get_entry_by_content(
        &self,
        content: &str,
        content_type: Option<&str>,
    ) -> Result<Option<i64>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let content_hash = if let Some(ct) = content_type {
            if is_text_type(ct) {
                calc_text_hash(content) as i64
            } else {
                0
            }
        } else {
            calc_text_hash(content) as i64
        };

        let result = if let Some(ct) = content_type {
            if ct == "image" {
                conn.query_row(
                    "SELECT id FROM clipboard_history
                     WHERE content_type = ?1 AND content_hash = ?2
                     ORDER BY timestamp DESC LIMIT 1",
                    params![ct, content_hash],
                    |row| row.get(0),
                )
            } else {
                let text_types = ["text", "code", "url", "rich_text"];
                conn.query_row(
                    "SELECT id FROM clipboard_history
                     WHERE content_type IN (SELECT value FROM json_each(?1)) AND content_hash = ?2
                     ORDER BY timestamp DESC LIMIT 1",
                    params![
                        serde_json::to_string(&text_types).unwrap_or_default(),
                        content_hash
                    ],
                    |row| row.get(0),
                )
            }
        } else {
            conn.query_row(
                "SELECT id FROM clipboard_history
                 WHERE content_hash = ?1 AND content_type NOT IN ('image')
                 ORDER BY timestamp DESC LIMIT 1",
                params![content_hash],
                |row| row.get(0),
            )
        };

        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn update_entry_content(&self, id: i64, content: &str, preview: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let content_hash = calc_text_hash(content) as i64;
        conn.execute(
            "UPDATE clipboard_history SET content = ?1, preview = ?2, content_hash = ?3 WHERE id = ?4",
            params![content, preview, content_hash, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get_entry_content(&self, id: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT content FROM clipboard_history WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(raw) => Ok(Some(self.maybe_decrypt_text(&raw))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn get_entry_content_full(&self, id: i64) -> Result<Option<(String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT content, preview FROM clipboard_history WHERE id = ?1",
            params![id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok((content_raw, preview_raw)) => Ok(Some((
                self.maybe_decrypt_text(&content_raw),
                self.maybe_decrypt_text(&preview_raw),
            ))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn get_entry_content_with_html(
        &self,
        id: i64,
    ) -> Result<Option<(String, String, Option<String>)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT content, preview, html_content FROM clipboard_history WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        );
        match result {
            Ok((content_raw, preview_raw, html_raw)) => Ok(Some((
                self.maybe_decrypt_text(&content_raw),
                self.maybe_decrypt_text(&preview_raw),
                html_raw.map(|v| self.maybe_decrypt_text(&v)),
            ))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}
