//! `harness-hook-no-find-grep` — tool.before hook that blocks bare
//! `find`/`grep` commands and common `rg` flag misuse.
//!
//! Registered on the `tool.before` window. Reads the pending tool
//! batch from stdin (JSON), inspects each `bash` call, and emits a
//! JSON decision on stdout:
//!
//! - `{}` — proceed (window default, no decision).
//! - `{"decision":"block","payload":{"reason":..., "calls":[...]}}`
//!   — block the listed calls; the loop synthesizes a failed
//!   `tool_result` for each.
//!
//! Ported from the Claude Code plugin `TonyWu20/no-find-grep` and
//! the pi extension `no-find-grep.ts`.

use std::io::Read;

use serde_json::json;

mod patterns;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let payload = read_stdin_json();

    // Not our window: no-op.
    if payload.get("window").and_then(|w| w.as_str()) != Some("tool.before") {
        println!("{}", json!({}));
        return;
    }

    let calls = payload
        .get("calls")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    let mut blocked: Vec<serde_json::Value> = Vec::new();

    for call in &calls {
        let name = call.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name != "bash" {
            continue;
        }

        let command = call
            .get("arguments")
            .and_then(|a| a.get("command"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        if let Some(reason) = patterns::check_command(command) {
            let id = call
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string();
            blocked.push(json!({
                "id": id,
                "reason": reason,
            }));
        }
    }

    if blocked.is_empty() {
        println!("{}", json!({}));
        return;
    }

    let ids: Vec<String> = blocked
        .iter()
        .map(|b| b.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string())
        .collect();
    let reasons: Vec<String> = blocked
        .iter()
        .map(|b| b.get("reason").and_then(|r| r.as_str()).unwrap_or("blocked").to_string())
        .collect();

    let resp = json!({
        "decision": "block",
        "payload": {
            "reason": reasons.join(" "),
            "calls": ids,
        }
    });
    println!("{}", resp);
}

fn read_stdin_json() -> serde_json::Value {
    let mut buf = String::new();
    if std::io::stdin()
        .read_to_string(&mut buf)
        .is_err()
        || buf.trim().is_empty()
    {
        return json!({});
    }
    serde_json::from_str(&buf).unwrap_or(json!({}))
}

fn print_help() {
    println!("harness-hook-no-find-grep — tool.before guard for find/grep/rg misuse");
    println!();
    println!("Window: tool.before");
    println!(
        "Input (stdin): {{window, session, calls:[{{id, name, arguments}}]}}"
    );
    println!("Output (stdout):");
    println!("  {{}}  — proceed (no violations)");
    println!(
        "  {{\"decision\":\"block\",\"payload\":{{\"reason\":...,\"calls\":[...]}}}} — block listed calls"
    );
    println!("Exit codes: 0 = ok");
}
