use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::database::{encryption, is_sensitive_key, ENCRYPT_PREFIX};

pub trait SettingsRepository {
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn get_all(&self) -> Result<HashMap<String, String>, String>;
    fn clear(&self) -> Result<(), String>;
    fn get_raw(&self, key: &str) -> Result<Option<String>, String>;
    fn set_raw(&self, key: &str, value: &str) -> Result<(), String>;
}

pub struct SqliteSettingsRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteSettingsRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    fn maybe_encrypt_value(&self, key: &str, value: &str) -> String {
        if is_sensitive_key(key) {
            if let Some(encrypted) = encryption::encrypt_value(value) {
                return encrypted;
            }
        }
        value.to_string()
    }

    fn maybe_decrypt_value(&self, key: &str, value: &str) -> String {
        if is_sensitive_key(key) && value.starts_with(ENCRYPT_PREFIX) {
            if let Some(decrypted) = encryption::decrypt_value(value) {
                return decrypted;
            }
        }
        value.to_string()
    }
}

impl SettingsRepository for SqliteSettingsRepository {
    fn get_raw(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set_raw(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let encrypted = self.maybe_encrypt_value(key, value);
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, encrypted],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(val) => Ok(Some(self.maybe_decrypt_value(key, &val))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn get_all(&self) -> Result<HashMap<String, String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })
            .map_err(|e| e.to_string())?;

        let mut map = HashMap::new();
        for row in rows {
            if let Ok((k, v)) = row {
                let decrypted = self.maybe_decrypt_value(&k, &v);
                map.insert(k, decrypted);
            }
        }
        Ok(map)
    }

    fn clear(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM settings", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
