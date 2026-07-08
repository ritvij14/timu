//! Folder listing — PRD §9 "Folder Picker".
//!
//! V0 lists the immediate subdirectories of a path over the existing SSH
//! connection (via [`crate::SshTransport::run_command`]), marking each as a Git
//! repo when a `.git` entry is present. Real SFTP (`russh-sftp`) is deferred;
//! the shell-command approach is portable (POSIX `test`) and fully testable
//! with [`crate::FakeSshTransport`].
//!
//! Build + parse are pure (mirroring `readiness_probe`); `list_folders` wires
//! them through the transport trait. User-supplied paths are single-quote
//! escaped via [`shell_quote`] to prevent shell injection.

use serde::{Deserialize, Serialize};

use crate::error::TimuError;
use crate::ssh::SshTransport;

/// One row in the folder picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderEntry {
    /// Full path of the directory, no trailing slash (e.g. `/home/u/projects/foo`).
    pub path: String,
    /// Base name of the directory (e.g. `foo`).
    pub name: String,
    /// True when a `.git` entry exists inside this directory.
    pub is_git_repo: bool,
}

/// Single-quote a string for safe interpolation into a shell command.
///
/// Wraps in `'...'` and rewrites embedded `'` as `'\''`. This is the standard
/// POSIX-safe quoting — no expansion happens inside single quotes.
pub fn shell_quote(s: &str) -> String {
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// Build the shell command that lists immediate subdirectories of `path`,
/// emitting `git:<path>/` or `dir:<path>/` per entry. Prints `ERR:notfound`
/// and exits 1 when `path` isn't a directory.
pub fn build_list_command(path: &str) -> String {
    let q = shell_quote(path);
    format!(
        "if [ ! -d {q} ]; then echo ERR:notfound; exit 1; fi; \
         for d in {q}/*/; do [ -d \"$d\" ] || continue; \
         if [ -d \"${{d}}.git\" ]; then echo \"git:$d\"; else echo \"dir:$d\"; fi; done"
    )
}

/// Parse `build_list_command` output into [`FolderEntry`]s. Malformed lines are
/// skipped. Pure — tested directly.
pub fn parse_list_entries(output: &str) -> Vec<FolderEntry> {
    let mut out = Vec::new();
    for raw in output.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let (is_git, path) = match line.split_once(':') {
            Some(("git", p)) => (true, p.trim()),
            Some(("dir", p)) => (false, p.trim()),
            _ => continue,
        };
        let path = path.trim_end_matches('/');
        if path.is_empty() {
            continue;
        }
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        out.push(FolderEntry {
            path: path.to_string(),
            name,
            is_git_repo: is_git,
        });
    }
    out
}

/// List immediate subdirectories of `path` over `transport`. Returns a typed
/// error when the directory doesn't exist (or the listing command fails).
pub async fn list_folders<T: SshTransport>(
    transport: &T,
    path: &str,
) -> Result<Vec<FolderEntry>, TimuError> {
    let cmd = build_list_command(path);
    let out = transport.run_command(&cmd).await?;
    if out.exit_code != 0 || out.stdout.contains("ERR:notfound") {
        return Err(TimuError::Other(format!("directory not found: {path}")));
    }
    Ok(parse_list_entries(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::FakeSshTransport;

    #[test]
    fn shell_quote_wraps_plain_string_in_single_quotes() {
        assert_eq!(shell_quote("foo"), "'foo'");
    }

    #[test]
    fn shell_quote_escapes_embedded_single_quotes() {
        // a'b -> 'a'\''b  (close quote, escaped quote, reopen)
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn shell_quote_neutralizes_injection_attempt() {
        let q = shell_quote("'; rm -rf /; echo");
        // The whole payload stays inside a single-quoted region — no expansion.
        assert!(q.starts_with('\''));
        assert!(q.ends_with('\''));
        assert!(!q.contains("\""));
    }

    #[test]
    fn build_list_command_quotes_path_and_checks_git() {
        let cmd = build_list_command("/home/u/projects");
        assert!(cmd.contains("'/home/u/projects'"));
        assert!(cmd.contains(".git"));
        assert!(cmd.contains("ERR:notfound"));
    }

    #[test]
    fn parse_marks_git_and_non_git_folders() {
        let out = "git:/home/u/projects/foo/\ndir:/home/u/projects/bar/\n";
        let entries = parse_list_entries(out);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "foo");
        assert_eq!(entries[0].path, "/home/u/projects/foo");
        assert!(entries[0].is_git_repo);
        assert_eq!(entries[1].name, "bar");
        assert!(!entries[1].is_git_repo);
    }

    #[test]
    fn parse_strips_trailing_slash_from_path() {
        let entries = parse_list_entries("dir:/x/y/\n");
        assert_eq!(entries[0].path, "/x/y");
        assert_eq!(entries[0].name, "y");
    }

    #[test]
    fn parse_empty_output_returns_empty_vec() {
        assert!(parse_list_entries("").is_empty());
    }

    #[test]
    fn parse_skips_malformed_lines() {
        let entries = parse_list_entries("garbage\ngit:/good/\n::\nfile:notafolder/\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "good");
        assert!(entries[0].is_git_repo);
    }

    #[tokio::test]
    async fn list_folders_via_fake_returns_git_and_non_git_entries() {
        let mut fake = FakeSshTransport::new();
        let path = "/home/u/projects";
        fake.script_success(
            build_list_command(path),
            "git:/home/u/projects/kendal-crm/\ndir:/home/u/projects/test-app/\n",
        );
        let entries = list_folders(&fake, path).await.expect("lists ok");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "kendal-crm");
        assert!(entries[0].is_git_repo);
        assert_eq!(entries[1].name, "test-app");
        assert!(!entries[1].is_git_repo);
    }

    #[tokio::test]
    async fn list_folders_returns_error_for_missing_directory() {
        let mut fake = FakeSshTransport::new();
        fake.script(
            build_list_command("/nope"),
            crate::ssh::CommandOutput::new("ERR:notfound\n", "ls: /nope: No such file or directory", 1),
        );
        let err = list_folders(&fake, "/nope").await.expect_err("should fail");
        assert_eq!(err.code(), "other");
        assert!(err.to_string().contains("directory not found"));
    }

    #[tokio::test]
    async fn list_folders_returns_error_on_nonzero_exit_without_marker() {
        // e.g. permission denied: ls exits nonzero, no ERR:notfound line.
        let mut fake = FakeSshTransport::new();
        fake.script(
            build_list_command("/restricted"),
            crate::ssh::CommandOutput::new(String::new(), "permission denied", 2),
        );
        let err = list_folders(&fake, "/restricted").await.expect_err("should fail");
        assert_eq!(err.code(), "other");
    }

    #[tokio::test]
    async fn list_folders_empty_directory_returns_empty_vec() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_list_command("/empty"), "");
        let entries = list_folders(&fake, "/empty").await.expect("ok");
        assert!(entries.is_empty());
    }

    #[test]
    fn folder_entry_round_trips_through_serde_json() {
        let e = FolderEntry {
            path: "/p/foo".into(),
            name: "foo".into(),
            is_git_repo: true,
        };
        let json = serde_json::to_string(&e).expect("serialize");
        let back: FolderEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(e, back);
    }
}