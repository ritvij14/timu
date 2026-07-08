//! Probe command builder + output parser — the pure seam between "ask the
//! machine what it has" and "turn the answer into a [`ReadinessReport`]".
//!
//! One round-trip over SSH runs [`build_probe_command`]; its stdout is fed to
//! [`parse_probe_output`]. Keeping build/parse pure and separate from the SSH
//! transport (see [`crate::ssh`]) means the whole readiness flow can be tested
//! with canned strings and no network.

use crate::readiness::{ReadinessReport, Tool, ToolStatus};

/// Shell script that prints `<tool>:<ready|missing>` for every tool the app
/// detects, in canonical order. Runs in a single SSH channel round-trip.
///
/// Uses `command -v` (POSIX) rather than `which` so it works on busybox/dash.
pub fn build_probe_command() -> String {
    let tools = Tool::all()
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "for t in {tools}; do if command -v \"$t\" >/dev/null 2>&1; then echo \"$t:ready\"; else echo \"$t:missing\"; fi; done"
    )
}

/// Parse the stdout produced by [`build_probe_command`] into a report.
///
/// Lines must look like `<tool>:<status>`. Unknown tool names and malformed
/// lines are silently skipped — a partial/empty probe yields [`ToolStatus::Unknown`]
/// for anything not mentioned, which is the intended degraded behavior.
pub fn parse_probe_output(output: &str) -> ReadinessReport {
    let mut report = ReadinessReport::new();
    for raw in output.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, status)) = line.split_once(':') else { continue };
        let Some(tool) = tool_from_str(name.trim()) else { continue };
        let status = match status.trim() {
            "ready" => ToolStatus::Ready,
            "missing" => ToolStatus::Missing,
            _ => continue,
        };
        report.set(tool, status);
    }
    report
}

/// Inverse of [`Tool::as_str`] for the probe protocol. Kept here (not on
/// `Tool`) because the protocol is this module's concern.
fn tool_from_str(s: &str) -> Option<Tool> {
    Tool::all().iter().copied().find(|t| t.as_str() == s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_command_mentions_every_tool_in_canonical_order() {
        let cmd = build_probe_command();
        let tools = Tool::all();
        // The `for t in ...` list is in canonical order.
        let list_start = cmd.find("for t in ").map(|i| i + "for t in ".len()).unwrap();
        let list_end = cmd[list_start..].find("; do").unwrap() + list_start;
        let names: Vec<&str> = cmd[list_start..list_end].split_whitespace().collect();
        let expected: Vec<&str> = tools.iter().map(|t| t.as_str()).collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn probe_command_uses_command_v_not_which() {
        let cmd = build_probe_command();
        assert!(cmd.contains("command -v"));
        assert!(!cmd.contains("which "));
    }

    #[test]
    fn parse_ready_and_missing_lines() {
        let out = "tmux:missing\ngit:ready\nnode:ready\npnpm:ready\ncodex:ready\nclaude:missing";
        let r = parse_probe_output(out);
        assert_eq!(r.get(Tool::Tmux), ToolStatus::Missing);
        assert_eq!(r.get(Tool::Git), ToolStatus::Ready);
        assert_eq!(r.get(Tool::Codex), ToolStatus::Ready);
        assert_eq!(r.get(Tool::Claude), ToolStatus::Missing);
        assert!(r.tmux_is_missing());
    }

    #[test]
    fn parse_empty_output_leaves_everything_unknown() {
        let r = parse_probe_output("");
        for t in Tool::all() {
            assert_eq!(r.get(*t), ToolStatus::Unknown, "{:?} not unknown", t);
        }
        assert!(!r.tmux_is_missing());
    }

    #[test]
    fn parse_skips_malformed_lines() {
        let r = parse_probe_output("garbage line\ntmux:missing\n::\nnotool:ready\n");
        assert_eq!(r.get(Tool::Tmux), ToolStatus::Missing);
        // "garbage line" has no colon, "::" has empty name, "notool" isn't a tool.
        assert_eq!(r.get(Tool::Git), ToolStatus::Unknown);
    }

    #[test]
    fn parse_ignores_unknown_status_values() {
        let r = parse_probe_output("tmux:maybe");
        assert_eq!(r.get(Tool::Tmux), ToolStatus::Unknown);
    }

    #[test]
    fn parse_ignores_unknown_tool_names() {
        let r = parse_probe_output("vim:ready\ntmux:missing");
        assert_eq!(r.get(Tool::Tmux), ToolStatus::Missing);
        // "vim" is not in our tool set; it must not pollute the report.
    }

    #[test]
    fn parse_tolerates_whitespace_around_fields() {
        let r = parse_probe_output("  tmux :  missing  \n git : ready ");
        assert_eq!(r.get(Tool::Tmux), ToolStatus::Missing);
        assert_eq!(r.get(Tool::Git), ToolStatus::Ready);
    }

    #[test]
    fn build_then_simulated_parse_round_trips_full_report() {
        // Simulate a machine that has everything except tmux and claude.
        let mut simulated = String::new();
        for t in Tool::all() {
            let status = match t {
                Tool::Tmux | Tool::Claude => "missing",
                _ => "ready",
            };
            simulated.push_str(t.as_str());
            simulated.push(':');
            simulated.push_str(status);
            simulated.push('\n');
        }
        let r = parse_probe_output(&simulated);
        assert!(r.tmux_is_missing());
        assert_eq!(r.get(Tool::Claude), ToolStatus::Missing);
        assert_eq!(r.get(Tool::Git), ToolStatus::Ready);
        assert_eq!(r.get(Tool::OpenCode), ToolStatus::Ready);
    }
}