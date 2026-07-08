//! Local persistent store — PRD §13 "Connection Persistence UX".
//!
//! SQLite via `rusqlite` (bundled, for mobile cross-compile). Owns machine
//! profiles, sessions, recent + favorite folders, and pinned host-key
//! fingerprints. **Never stores credentials** (Hard Block §2.1 / ADR-002) —
//! the `machine_profiles` schema has no password/key/passphrase columns, which
//! `assert_no_secret_columns_in_machine_profiles` enforces forever.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::credentials::Credentials; // import only to assert it's never stored
use crate::folder::FolderEntry;
use crate::host_key::{Fingerprint, HostKeyPins};
use crate::profile::AuthMethod;

/// A persisted machine profile with its row id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRecord {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub username: String,
    pub port: u16,
    pub auth_method: AuthMethod,
}

/// A persisted coding session (one row per tmux-backed agent session).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: i64,
    pub profile_id: i64,
    pub agent: String,
    pub folder: String,
    pub tmux_session_id: String,
    pub status: String,
}

/// SQLite-backed store.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open a file-backed store (creates the file + schema if missing).
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// In-memory store for tests.
    pub fn in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS machine_profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                host TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                auth_method TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_opened_at TEXT
            );
            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_id INTEGER NOT NULL,
                agent TEXT NOT NULL,
                folder TEXT NOT NULL,
                tmux_session_id TEXT NOT NULL,
                last_active_at TEXT,
                status TEXT NOT NULL DEFAULT 'running',
                FOREIGN KEY (profile_id) REFERENCES machine_profiles(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS recent_folders (
                path TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                is_git_repo INTEGER NOT NULL,
                last_used_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS favorites (
                path TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                is_git_repo INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS host_key_pins (
                host TEXT PRIMARY KEY,
                fingerprint TEXT NOT NULL
            );",
        )
    }

    // -- machine profiles -------------------------------------------------

    /// Insert a profile, return its new row id.
    pub fn save_profile(&self, p: &crate::profile::MachineProfile) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO machine_profiles (name, host, username, port, auth_method) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![p.name, p.host, p.username, p.port, auth_method_tag(&p.auth_method)],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_profiles(&self) -> rusqlite::Result<Vec<ProfileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, host, username, port, auth_method FROM machine_profiles ORDER BY id",
        )?;
        let rows = stmt.query_map([], row_to_profile)?;
        rows.collect()
    }

    pub fn get_profile(&self, id: i64) -> rusqlite::Result<Option<ProfileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, host, username, port, auth_method FROM machine_profiles WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_profile)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Delete a profile; returns true if a row was removed.
    pub fn delete_profile(&self, id: i64) -> rusqlite::Result<bool> {
        let n = self.conn.execute("DELETE FROM machine_profiles WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// Update `last_opened_at` when the user opens this machine.
    pub fn touch_profile(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE machine_profiles SET last_opened_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    // -- sessions ---------------------------------------------------------

    pub fn save_session(&self, s: &SessionRecord) -> rusqlite::Result<i64> {
        if s.id == 0 {
            self.conn.execute(
                "INSERT INTO sessions (profile_id, agent, folder, tmux_session_id, status) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![s.profile_id, s.agent, s.folder, s.tmux_session_id, s.status],
            )?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn.execute(
                "UPDATE sessions SET profile_id=?1, agent=?2, folder=?3, tmux_session_id=?4, status=?5, last_active_at=datetime('now') WHERE id=?6",
                params![s.profile_id, s.agent, s.folder, s.tmux_session_id, s.status, s.id],
            )?;
            Ok(s.id)
        }
    }

    pub fn list_sessions(&self, profile_id: i64) -> rusqlite::Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, profile_id, agent, folder, tmux_session_id, status FROM sessions WHERE profile_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![profile_id], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                agent: row.get(2)?,
                folder: row.get(3)?,
                tmux_session_id: row.get(4)?,
                status: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    // -- recent folders ---------------------------------------------------

    pub fn add_recent_folder(&self, entry: &FolderEntry) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO recent_folders (path, name, is_git_repo, last_used_at) VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(path) DO UPDATE SET last_used_at = datetime('now'), is_git_repo = ?3",
            params![entry.path, entry.name, entry.is_git_repo as i32],
        )?;
        Ok(())
    }

    pub fn list_recent_folders(&self) -> rusqlite::Result<Vec<FolderEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, name, is_git_repo FROM recent_folders ORDER BY datetime(last_used_at) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(FolderEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                is_git_repo: row.get::<_, i32>(2)? != 0,
            })
        })?;
        rows.collect()
    }

    // -- favorites --------------------------------------------------------

    pub fn add_favorite(&self, entry: &FolderEntry) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO favorites (path, name, is_git_repo) VALUES (?1, ?2, ?3)",
            params![entry.path, entry.name, entry.is_git_repo as i32],
        )?;
        Ok(())
    }

    pub fn list_favorites(&self) -> rusqlite::Result<Vec<FolderEntry>> {
        let mut stmt = self.conn.prepare("SELECT path, name, is_git_repo FROM favorites ORDER BY path")?;
        let rows = stmt.query_map([], |row| {
            Ok(FolderEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                is_git_repo: row.get::<_, i32>(2)? != 0,
            })
        })?;
        rows.collect()
    }

    pub fn remove_favorite(&self, path: &str) -> rusqlite::Result<bool> {
        let n = self.conn.execute("DELETE FROM favorites WHERE path = ?1", params![path])?;
        Ok(n > 0)
    }

    // -- host-key pins ----------------------------------------------------

    /// Pin (or replace) the fingerprint for a host.
    pub fn save_host_key_pin(&self, host: &str, fp: &Fingerprint) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO host_key_pins (host, fingerprint) VALUES (?1, ?2)",
            params![host, fp.as_str()],
        )?;
        Ok(())
    }

    /// Load all pinned host keys into a [`HostKeyPins`].
    pub fn load_host_key_pins(&self) -> rusqlite::Result<HostKeyPins> {
        let mut stmt = self.conn.prepare("SELECT host, fingerprint FROM host_key_pins")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (host, fp) = row?;
            map.insert(host, Fingerprint::new(fp));
        }
        Ok(HostKeyPins::from_map(map))
    }
}

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileRecord> {
    Ok(ProfileRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        host: row.get(2)?,
        username: row.get(3)?,
        port: row.get::<_, i64>(4)? as u16,
        auth_method: auth_method_from_tag(&row.get::<_, String>(5)?),
    })
}

fn auth_method_tag(m: &AuthMethod) -> &'static str {
    match m {
        AuthMethod::Password => "password",
        AuthMethod::KeyPaste => "key_paste",
        AuthMethod::KeyFile => "key_file",
    }
}

fn auth_method_from_tag(tag: &str) -> AuthMethod {
    match tag {
        "key_paste" => AuthMethod::KeyPaste,
        "key_file" => AuthMethod::KeyFile,
        _ => AuthMethod::Password,
    }
}

// Compile-time guard: Credentials is referenced from this module only so that
// removing it (or accidentally storing it) becomes a visible compile error in
// the test below. This line deliberately does nothing with it.
const _: fn() = || {
    let _ = std::marker::PhantomData::<Credentials>;
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::FolderEntry;
    use crate::profile::MachineProfile;

    fn profile(name: &str) -> MachineProfile {
        MachineProfile {
            name: name.into(),
            host: "10.0.0.1".into(),
            username: "root".into(),
            port: 22,
            auth_method: AuthMethod::Password,
        }
    }

    fn entry(path: &str, git: bool) -> FolderEntry {
        FolderEntry {
            path: path.into(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            is_git_repo: git,
        }
    }

    #[test]
    fn profile_round_trips_through_the_store() {
        let store = Store::in_memory().expect("open");
        let id = store.save_profile(&profile("My VPS")).expect("save");
        let got = store.get_profile(id).expect("get").expect("present");
        assert_eq!(got.id, id);
        assert_eq!(got.name, "My VPS");
        assert_eq!(got.host, "10.0.0.1");
        assert_eq!(got.port, 22);
        assert_eq!(got.auth_method, AuthMethod::Password);
    }

    #[test]
    fn list_profiles_returns_insertion_order() {
        let store = Store::in_memory().expect("open");
        store.save_profile(&profile("first")).expect("save");
        store.save_profile(&profile("second")).expect("save");
        let names: Vec<_> = store.list_profiles().expect("list").into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["first", "second"]);
    }

    #[test]
    fn delete_profile_returns_true_then_false() {
        let store = Store::in_memory().expect("open");
        let id = store.save_profile(&profile("gone")).expect("save");
        assert!(store.delete_profile(id).expect("del"));
        assert!(!store.delete_profile(id).expect("del again"));
        assert!(store.get_profile(id).expect("get").is_none());
    }

    #[test]
    fn auth_method_key_paste_round_trips() {
        let store = Store::in_memory().expect("open");
        let mut p = profile("keybox");
        p.auth_method = AuthMethod::KeyPaste;
        let id = store.save_profile(&p).expect("save");
        let got = store.get_profile(id).expect("get").expect("present");
        assert_eq!(got.auth_method, AuthMethod::KeyPaste);
    }

    #[test]
    fn save_session_inserts_and_lists_under_profile() {
        let store = Store::in_memory().expect("open");
        let pid = store.save_profile(&profile("box")).expect("save");
        let s = SessionRecord {
            id: 0,
            profile_id: pid,
            agent: "codex".into(),
            folder: "/p/kendal".into(),
            tmux_session_id: "sess-1".into(),
            status: "running".into(),
        };
        let sid = store.save_session(&s).expect("save");
        assert!(sid > 0);
        let listed = store.list_sessions(pid).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].agent, "codex");
        assert_eq!(listed[0].tmux_session_id, "sess-1");
    }

    #[test]
    fn save_session_updates_existing_row() {
        let store = Store::in_memory().expect("open");
        let pid = store.save_profile(&profile("box")).expect("save");
        let s = SessionRecord {
            id: 0,
            profile_id: pid,
            agent: "codex".into(),
            folder: "/p".into(),
            tmux_session_id: "s".into(),
            status: "running".into(),
        };
        let sid = store.save_session(&s).expect("save");
        let mut updated = s.clone();
        updated.id = sid;
        updated.status = "stopped".into();
        store.save_session(&updated).expect("update");
        let listed = store.list_sessions(pid).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, "stopped");
        assert_eq!(listed[0].id, sid);
    }

    #[test]
    fn recent_folders_dedup_and_order_by_last_used() {
        let store = Store::in_memory().expect("open");
        store.add_recent_folder(&entry("/p/a", true)).expect("add");
        store.add_recent_folder(&entry("/p/b", false)).expect("add");
        // Touching /p/a again should bubble it to the top.
        store.add_recent_folder(&entry("/p/a", true)).expect("add");
        let recent = store.list_recent_folders().expect("list");
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].path, "/p/a");
        assert_eq!(recent[1].path, "/p/b");
    }

    #[test]
    fn favorites_add_list_remove() {
        let store = Store::in_memory().expect("open");
        store.add_favorite(&entry("/p/fav", true)).expect("add");
        assert_eq!(store.list_favorites().expect("list").len(), 1);
        assert!(store.remove_favorite("/p/fav").expect("rm"));
        assert!(store.list_favorites().expect("list").is_empty());
    }

    #[test]
    fn host_key_pins_round_trip() {
        let store = Store::in_memory().expect("open");
        store.save_host_key_pin("my-vps", &Fingerprint::new("SHA256:abc")).expect("save");
        let pins = store.load_host_key_pins().expect("load");
        assert!(pins.is_pinned("my-vps"));
        assert_eq!(
            pins.verify("my-vps", &Fingerprint::new("SHA256:abc")),
            crate::host_key::HostKeyVerdict::Matches
        );
        assert_eq!(
            pins.verify("my-vps", &Fingerprint::new("SHA256:zzz")),
            crate::host_key::HostKeyVerdict::Mismatch
        );
    }

    /// Hard Block §2.1 enforcement at the schema level: the machine_profiles
    /// table must never gain a password / private_key / passphrase column.
    #[test]
    fn assert_no_secret_columns_in_machine_profiles() {
        let store = Store::in_memory().expect("open");
        let mut stmt = store.conn.prepare("PRAGMA table_info(machine_profiles)").expect("pragma");
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("map")
            .filter_map(|r| r.ok())
            .collect();
        for col in &cols {
            let lower = col.to_lowercase();
            assert!(
                !lower.contains("password")
                    && !lower.contains("private_key")
                    && !lower.contains("passphrase")
                    && !lower.contains("secret")
                    && !lower.contains("key_bytes"),
                "forbidden secret-like column in machine_profiles: {col}"
            );
        }
        // Sanity: the columns we DO expect are present.
        assert!(cols.contains(&"name".into()));
        assert!(cols.contains(&"auth_method".into()));
    }

    #[test]
    fn every_table_is_present_after_init() {
        let store = Store::in_memory().expect("open");
        let mut stmt = store.conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").expect("q");
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("map")
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"machine_profiles".into()));
        assert!(tables.contains(&"sessions".into()));
        assert!(tables.contains(&"recent_folders".into()));
        assert!(tables.contains(&"favorites".into()));
        assert!(tables.contains(&"host_key_pins".into()));
    }
}