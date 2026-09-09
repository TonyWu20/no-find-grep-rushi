//! Pure regex checks for bare find/grep and rg flag misuse.
//!
//! Ported from the Claude Code plugin `block-find-grep.sh` and the
//! pi extension `no-find-grep.ts`. Each function takes a shell
//! command string and returns `Some(reason)` when the command is
//! blocked, `None` when it passes.
//!
//! The regex patterns are the same as the TS port. The Rust `regex`
//! crate has no lookahead, so `(?=\s|$)` becomes `(\s|$)`. The
//! match semantics are equivalent for `is_match()`.

use regex::Regex;
use std::sync::OnceLock;

// ── Regex patterns (ported from block-find-grep.sh / no-find-grep.ts) ────

/// Bare `find` at command start or after pipe/separator.
fn bare_find() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(^|[|;&$()]+\s*)find(\s|$)").unwrap())
}

/// Bare `grep` at command start or after pipe/separator.
fn bare_grep() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(^|[|;&$()]+\s*)grep(\s|$)").unwrap())
}

/// Capture the first pipeline segment that starts with `rg`.
/// Allows `\|` (escaped pipe) within the segment so the flag
/// checks below can see it.
fn rg_segment() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|[|;&$()]+\s*)rg\b(?:\\\||[^|;&$()])*").unwrap())
}

/// `rg -r<letter>`: `-r` means `--replace` in rg, not `--recursive`.
fn rg_r_flag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)-r[a-zA-Z]").unwrap())
}

/// `rg -L` standalone: `-L` means `--follow` in rg, not
/// `--files-without-match`.
fn rg_l_flag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)-L(?:\s|$)").unwrap())
}

/// `rg \|`: escaped pipe matches a literal pipe in rg; use `|`
/// for alternation.
fn rg_escaped_pipe() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\\\|").unwrap())
}

// ── Check functions ─────────────────────────────────────────────────────

/// Check for bare `find` in a shell command.
pub fn check_bare_find(cmd: &str) -> Option<String> {
    if cmd.is_empty() {
        return None;
    }
    if bare_find().is_match(cmd) {
        return Some(format!(
            "BARE find DETECTED! Use fd (or fdfind on Debian/Ubuntu) instead of find!\n\
             Your command: {cmd}\n\
             Fix: replace 'find' with 'fd' (or 'fdfind' on Debian/Ubuntu)"
        ));
    }
    None
}

/// Check for bare `grep` in a shell command.
pub fn check_bare_grep(cmd: &str) -> Option<String> {
    if cmd.is_empty() {
        return None;
    }
    if bare_grep().is_match(cmd) {
        return Some(format!(
            "BARE grep DETECTED! Use rg instead of grep!\n\
             Your command: {cmd}\n\
             Fix: replace 'grep' with 'rg'"
        ));
    }
    None
}

/// Check for `rg` flag misuse: `-r<letter>`, standalone `-L`,
/// and escaped pipe `\|`. Only scoped to the `rg` segment.
pub fn check_rg_misuse(cmd: &str) -> Option<String> {
    if cmd.is_empty() {
        return None;
    }
    let Some(m) = rg_segment().find(cmd) else {
        return None;
    };
    let seg = &cmd[m.start()..m.end()];

    // rg -r<letter>: -r means --replace, not --recursive
    if rg_r_flag().is_match(seg) {
        return Some(format!(
            "grep -r FLAG DETECTED IN rg COMMAND!\n\
             In rg, -r means --replace, NOT --recursive like grep.\n\
             rg is ALREADY recursive by default — no -r flag needed.\n\
             Your command: {cmd}\n\
             Fix: remove the -r flag. rg searches recursively by default."
        ));
    }

    // rg -L: -L means --follow, not --files-without-match
    if rg_l_flag().is_match(seg) {
        return Some(format!(
            "grep -L FLAG DETECTED IN rg COMMAND!\n\
             In rg, -L means --follow, NOT --files-without-match!\n\
             Your command: {cmd}\n\
             Fix: use 'rg --files-without-match' for grep -L behavior"
        ));
    }

    // rg \|: escaped pipe matches literal pipe
    if rg_escaped_pipe().is_match(seg) {
        return Some(format!(
            "ESCAPED PIPE IN rg DETECTED! \\| means literal pipe in rg!\n\
             rg uses | for alternation, NOT \\|!\n\
             Your command: {cmd}\n\
             Fix: replace '\\|' with '|' in your rg pattern"
        ));
    }

    None
}

/// Run all checks on a command. Returns the first matching reason.
pub fn check_command(cmd: &str) -> Option<String> {
    check_bare_find(cmd)
        .or_else(|| check_bare_grep(cmd))
        .or_else(|| check_rg_misuse(cmd))
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bare find ──

    #[test]
    fn find_at_start() {
        let r = check_bare_find("find . -name '*.txt'");
        assert!(r.is_some());
        assert!(r.unwrap().contains("BARE find"));
    }

    #[test]
    fn find_after_pipe() {
        assert!(check_bare_find("ls | find .").is_some());
    }

    #[test]
    fn find_after_semicolon() {
        assert!(check_bare_find("cmd; find .").is_some());
    }

    #[test]
    fn find_after_and() {
        assert!(check_bare_find("cmd && find .").is_some());
    }

    #[test]
    fn find_in_subshell() {
        assert!(check_bare_find("(find .)").is_some());
    }

    #[test]
    fn find_as_argument_not_blocked() {
        assert!(check_bare_find("echo find").is_none());
        assert!(check_bare_find("echo 'find'").is_none());
    }

    #[test]
    fn find_inside_word_not_blocked() {
        assert!(check_bare_find("sfind .").is_none());
    }

    #[test]
    fn fd_not_blocked() {
        assert!(check_bare_find("fd -t f .").is_none());
        assert!(check_bare_find("fdfind -name '*.txt' .").is_none());
    }

    #[test]
    fn empty_command_not_blocked() {
        assert!(check_bare_find("").is_none());
    }

    // ── Bare grep ──

    #[test]
    fn grep_at_start() {
        let r = check_bare_grep("grep -rn 'pattern' .");
        assert!(r.is_some());
        assert!(r.unwrap().contains("BARE grep"));
    }

    #[test]
    fn grep_after_pipe() {
        assert!(check_bare_grep("cat file | grep foo").is_some());
    }

    #[test]
    fn grep_in_subshell() {
        assert!(check_bare_grep("(grep foo)").is_some());
    }

    #[test]
    fn grep_after_semicolon() {
        assert!(check_bare_grep("cmd; grep foo").is_some());
    }

    #[test]
    fn git_grep_not_blocked() {
        assert!(check_bare_grep("git grep foo").is_none());
    }

    #[test]
    fn grep_as_argument_not_blocked() {
        assert!(check_bare_grep("echo grep").is_none());
    }

    // ── rg -r flag ──

    #[test]
    fn rg_rn_flag_blocked() {
        let r = check_rg_misuse("rg -rn 'pattern' .");
        assert!(r.is_some());
        assert!(r.unwrap().contains("-r"));
    }

    #[test]
    fn rg_r_flag_scoped_to_rg_segment() {
        // -r on a piped grep is not rg's flag: not blocked.
        assert!(check_rg_misuse("rg 'pattern' . | xargs -r cmd").is_none());
    }

    #[test]
    fn rg_clean_usage_not_blocked() {
        assert!(check_rg_misuse("rg 'pattern' .").is_none());
    }

    #[test]
    fn rg_recursive_flag_long_form_not_blocked() {
        // --recursive is the correct long form; not blocked.
        assert!(check_rg_misuse("rg --recursive 'pattern' .").is_none());
    }

    // ── rg -L flag ──

    #[test]
    fn rg_l_flag_blocked() {
        let r = check_rg_misuse("rg -L 'pattern' .");
        assert!(r.is_some());
        assert!(r.unwrap().contains("-L"));
    }

    #[test]
    fn rg_files_without_match_not_blocked() {
        assert!(check_rg_misuse("rg --files-without-match 'pattern' .").is_none());
    }

    // ── rg escaped pipe ──

    #[test]
    fn rg_escaped_pipe_blocked() {
        let r = check_rg_misuse("rg 'a\\|b' .");
        assert!(r.is_some());
        assert!(r.unwrap().contains("ESCAPED PIPE"));
    }

    #[test]
    fn rg_alternation_pipe_not_blocked() {
        assert!(check_rg_misuse("rg 'a|b' .").is_none());
    }

    // ── Combined check_command ──

    #[test]
    fn check_command_blocks_bare_find() {
        assert!(check_command("find . -name '*.txt'").is_some());
    }

    #[test]
    fn check_command_blocks_bare_grep() {
        assert!(check_command("grep -rn 'pattern' .").is_some());
    }

    #[test]
    fn check_command_blocks_rg_misuse() {
        assert!(check_command("rg -rn 'pattern' .").is_some());
    }

    #[test]
    fn check_command_allows_clean_commands() {
        assert!(check_command("fd -t f .").is_none());
        assert!(check_command("rg 'pattern' .").is_none());
        assert!(check_command("ls -la").is_none());
        assert!(check_command("cargo build").is_none());
    }

    #[test]
    fn check_command_empty_string() {
        assert!(check_command("").is_none());
    }
}
