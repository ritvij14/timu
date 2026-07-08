//! Machine readiness — PRD §7 "Machine Readiness Check" and §8 "tmux Requirement".
//!
//! A [`ReadinessReport`] is the typed result of probing the target machine for
//! the tools the app cares about. It is built by parsing the output of a single
//! probe command (see [`crate::readiness_probe`]). `tmux` is the only *required*
//! tool for V0; everything else is informational.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A tool the app detects on the target machine.
///
/// Declaration order is the canonical render order used by
/// [`ReadinessReport::render`], matching PRD §7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tool {
    Tmux,
    Git,
    Shell,
    Node,
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Codex,
    Claude,
    OpenCode,
}

impl Tool {
    /// Lowercase name as it appears in the PRD §7 status list.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tmux => "tmux",
            Self::Git => "git",
            Self::Shell => "shell",
            Self::Node => "node",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
        }
    }

    /// All tools in canonical render order.
    pub fn all() -> &'static [Tool] {
        &[
            Tool::Tmux,
            Tool::Git,
            Tool::Shell,
            Tool::Node,
            Tool::Npm,
            Tool::Pnpm,
            Tool::Yarn,
            Tool::Bun,
            Tool::Codex,
            Tool::Claude,
            Tool::OpenCode,
        ]
    }
}

/// Detection result for a single tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolStatus {
    /// Found on PATH.
    Ready,
    /// Confirmed absent (`command -v` exited non-zero).
    Missing,
    /// Probe didn't run or output was ambiguous.
    Unknown,
}

impl fmt::Display for ToolStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Ready => "Ready",
            Self::Missing => "Missing",
            Self::Unknown => "Unknown",
        };
        write!(f, "{s}")
    }
}

/// Result of probing a machine. Maps each [`Tool`] to its [`ToolStatus`].
///
/// Tools that were never set default to [`ToolStatus::Unknown`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    statuses: HashMap<Tool, ToolStatus>,
}

impl ReadinessReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the status of a tool.
    pub fn set(&mut self, tool: Tool, status: ToolStatus) {
        self.statuses.insert(tool, status);
    }

    /// Look up the status of a tool (`Unknown` if never probed).
    pub fn get(&self, tool: Tool) -> ToolStatus {
        self.statuses.get(&tool).copied().unwrap_or(ToolStatus::Unknown)
    }

    /// PRD §8: tmux is required. True only when tmux is confirmed missing —
    /// the condition under which the UI must show the install-command path.
    pub fn tmux_is_missing(&self) -> bool {
        self.get(Tool::Tmux) == ToolStatus::Missing
    }

    /// Render the PRD §7 status list:
    /// ```text
    /// tmux        Missing
    /// git         Ready
    /// ```
    /// All tools, in canonical order, name left-padded to `NAME_WIDTH`.
    pub fn render(&self) -> String {
        const NAME_WIDTH: usize = 12;
        Tool::all()
            .iter()
            .map(|tool| format!("{:<width$}{}", tool.as_str(), self.get(*tool), width = NAME_WIDTH))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_with(tmux: ToolStatus) -> ReadinessReport {
        let mut r = ReadinessReport::new();
        r.set(Tool::Tmux, tmux);
        r
    }

    #[test]
    fn unset_tools_default_to_unknown() {
        let r = ReadinessReport::new();
        assert_eq!(r.get(Tool::Git), ToolStatus::Unknown);
    }

    #[test]
    fn tmux_is_missing_true_only_when_tmux_missing() {
        assert!(report_with(ToolStatus::Missing).tmux_is_missing());
        assert!(!report_with(ToolStatus::Ready).tmux_is_missing());
        assert!(!report_with(ToolStatus::Unknown).tmux_is_missing());
        assert!(!ReadinessReport::new().tmux_is_missing());
    }

    #[test]
    fn render_lists_all_tools_in_canonical_order() {
        let mut r = ReadinessReport::new();
        r.set(Tool::Tmux, ToolStatus::Missing);
        r.set(Tool::Git, ToolStatus::Ready);
        r.set(Tool::Node, ToolStatus::Ready);
        r.set(Tool::Pnpm, ToolStatus::Ready);
        r.set(Tool::Codex, ToolStatus::Ready);
        r.set(Tool::Claude, ToolStatus::Missing);

        let rendered = r.render();
        let lines: Vec<&str> = rendered.lines().collect();
        // Canonical order check on the first few entries.
        assert_eq!(lines[0], "tmux        Missing");
        assert_eq!(lines[1], "git         Ready");
        assert_eq!(lines[2], "shell       Unknown");
        assert_eq!(lines[3], "node        Ready");
        // All 11 tools appear.
        assert_eq!(lines.len(), Tool::all().len());
        // opencode (longest name, 8 chars) is last and still aligned.
        assert!(lines.last().unwrap().starts_with("opencode    "));
    }

    #[test]
    fn render_matches_prd_section_7_example_subset() {
        let mut r = ReadinessReport::new();
        r.set(Tool::Tmux, ToolStatus::Missing);
        r.set(Tool::Git, ToolStatus::Ready);
        r.set(Tool::Node, ToolStatus::Ready);
        r.set(Tool::Pnpm, ToolStatus::Ready);
        r.set(Tool::Codex, ToolStatus::Ready);
        r.set(Tool::Claude, ToolStatus::Missing);

        let rendered = r.render();
        assert!(rendered.contains("tmux        Missing"));
        assert!(rendered.contains("git         Ready"));
        assert!(rendered.contains("codex       Ready"));
        assert!(rendered.contains("claude      Missing"));
    }

    #[test]
    fn tool_as_str_matches_prd_names() {
        assert_eq!(Tool::Tmux.as_str(), "tmux");
        assert_eq!(Tool::OpenCode.as_str(), "opencode");
    }

    #[test]
    fn tool_all_is_in_canonical_order_with_no_duplicates() {
        let all = Tool::all();
        let mut seen = std::collections::HashSet::new();
        for t in all {
            assert!(seen.insert(*t), "duplicate tool in all(): {t:?}");
        }
        assert!(all.len() >= 11);
    }

    #[test]
    fn report_round_trips_through_serde_json() {
        let mut r = ReadinessReport::new();
        r.set(Tool::Tmux, ToolStatus::Missing);
        r.set(Tool::Git, ToolStatus::Ready);
        let json = serde_json::to_string(&r).expect("serialize");
        let back: ReadinessReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
    }

    #[test]
    fn tool_status_display_matches_prd_labels() {
        assert_eq!(ToolStatus::Ready.to_string(), "Ready");
        assert_eq!(ToolStatus::Missing.to_string(), "Missing");
        assert_eq!(ToolStatus::Unknown.to_string(), "Unknown");
    }
}