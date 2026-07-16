use rusqlite::{params, Connection, Result};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct StickyNote {
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

pub trait StickyRepository {
    fn create(&self, note: &StickyNote) -> Result<i64>;
    fn update(&self, note: &StickyNote) -> Result<()>;
    fn delete(&self, id: i64) -> Result<()>;
    fn get_all(&self) -> Result<Vec<StickyNote>>;
    fn get_by_id(&self, id: i64) -> Result<Option<StickyNote>>;
}

pub struct SqliteStickyRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStickyRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

impl StickyRepository for SqliteStickyRepository {
    fn create(&self, note: &StickyNote) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sticky_windows (content, content_type, x, y, width, height, always_on_top, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                note.content,
                note.content_type,
                note.x,
                note.y,
                note.width,
                note.height,
                note.always_on_top as i32,
                note.created_at,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn update(&self, note: &StickyNote) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sticky_windows SET content = ?1, content_type = ?2, x = ?3, y = ?4,
             width = ?5, height = ?6, always_on_top = ?7 WHERE id = ?8",
            params![
                note.content,
                note.content_type,
                note.x,
                note.y,
                note.width,
                note.height,
                note.always_on_top as i32,
                note.id,
            ],
        )?;
        Ok(())
    }

    fn delete(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sticky_windows WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn get_all(&self) -> Result<Vec<StickyNote>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content, content_type, x, y, width, height, always_on_top, created_at
             FROM sticky_windows ORDER BY created_at DESC",
        )?;
        let notes = stmt
            .query_map([], |row| {
                Ok(StickyNote {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    content_type: row.get(2)?,
                    x: row.get(3)?,
                    y: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    always_on_top: row.get::<_, i32>(7)? != 0,
                    created_at: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(notes)
    }

    fn get_by_id(&self, id: i64) -> Result<Option<StickyNote>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, content, content_type, x, y, width, height, always_on_top, created_at
             FROM sticky_windows WHERE id = ?1",
            params![id],
            |row| {
                Ok(StickyNote {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    content_type: row.get(2)?,
                    x: row.get(3)?,
                    y: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    always_on_top: row.get::<_, i32>(7)? != 0,
                    created_at: row.get(8)?,
                })
            },
        );
        match result {
            Ok(note) => Ok(Some(note)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
