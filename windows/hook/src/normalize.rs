//! Every agent's hook payload, mapped onto the events and fields the island
//! knows (Claude Code's). Same table as `normalize_event` /
//! `normalize_tool_fields` in the Mac relay (HookServer.swift), plus Cursor's
//! events.

use serde_json::{Map, Value};

/// The canonical name of an agent's event. Unknown names pass through.
pub fn event(name: &str) -> &str {
    match name {
        // Gemini CLI
        "BeforeTool" | "BeforeToolSelection" => "PreToolUse",
        "AfterTool" | "AfterModel" => "PostToolUse",
        "BeforeAgent" => "UserPromptSubmit",
        "AfterAgent" => "Stop",
        "startup" => "SessionStart",
        "exit" => "SessionEnd",
        // Antigravity
        "PreInvocation" => "UserPromptSubmit",
        "PostInvocation" => "PostToolUse",
        // snake_case relays
        "pre_tool_use" => "PreToolUse",
        "post_tool_use" => "PostToolUse",
        "user_prompt_submit" => "UserPromptSubmit",
        "session_start" => "SessionStart",
        "session_end" => "SessionEnd",
        "stop" => "Stop",
        // Copilot CLI (camelCase) — Cursor shares most of these
        "sessionStart" => "SessionStart",
        "userPromptSubmitted" => "UserPromptSubmit",
        "agentStop" => "Stop",
        "notification" => "Notification",
        "preToolUse" => "PreToolUse",
        "postToolUse" => "PostToolUse",
        "permissionRequest" => "PermissionRequest",
        "sessionEnd" => "SessionEnd",
        // Cursor
        "beforeSubmitPrompt" => "UserPromptSubmit",
        "postToolUseFailure" => "PostToolUseFailure",
        "subagentStart" => "SubagentStart",
        "subagentStop" => "SubagentStop",
        other => other,
    }
}

/// Keys of Antigravity's `toolCall.args` the island reads under another name.
const ARG_ALIASES: &[(&str, &str)] = &[
    ("CommandLine", "command"),
    ("FilePath", "file_path"),
    ("Path", "path"),
    ("Url", "url"),
    ("Query", "query"),
    ("Pattern", "pattern"),
];

fn non_empty_str<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Fills `tool_name`, `tool_input`, `session_id` and `cwd` from wherever the
/// agent put them. A field the payload already has is never overwritten.
/// `env` reads an environment variable (a parameter so tests stay pure).
pub fn fields(map: &mut Map<String, Value>, env: &dyn Fn(&str) -> Option<String>) {
    let tool_call = map.get("toolCall").and_then(Value::as_object).cloned();

    if !map.contains_key("tool_name") {
        // Copilot: toolName. Gemini CLI / Antigravity: toolCall.name. Others: tool.
        let name = non_empty_str(map, "toolName")
            .or_else(|| tool_call.as_ref().and_then(|t| non_empty_str(t, "name")))
            .or_else(|| non_empty_str(map, "tool"))
            .map(str::to_string);
        if let Some(name) = name {
            map.insert("tool_name".into(), Value::String(name));
        }
    }

    if !map.contains_key("tool_input") {
        // Copilot sends toolArgs as an object, or as the JSON text of one.
        let args = match map.get("toolArgs") {
            Some(Value::Object(o)) => Some(o.clone()),
            Some(Value::String(s)) => serde_json::from_str::<Value>(s)
                .ok()
                .and_then(|v| v.as_object().cloned()),
            _ => None,
        };
        let input = args.or_else(|| {
            let mut flat = tool_call.as_ref()?.get("args")?.as_object()?.clone();
            for (from, to) in ARG_ALIASES {
                if let Some(v) = flat.get(*from).cloned() {
                    flat.insert((*to).to_string(), v);
                }
            }
            Some(flat)
        });
        if let Some(input) = input {
            map.insert("tool_input".into(), Value::Object(input));
        }
    }

    if non_empty_str(map, "session_id").is_none() {
        let sid = ["conversationId", "conversation_id", "sessionId", "GEMINI_SESSION_ID"]
            .iter()
            .find_map(|k| non_empty_str(map, k))
            .map(str::to_string)
            .or_else(|| env("GEMINI_SESSION_ID").filter(|s| !s.is_empty()));
        if let Some(sid) = sid {
            map.insert("session_id".into(), Value::String(sid));
        }
    }

    if non_empty_str(map, "cwd").is_none() {
        // Copilot: workdir. Gemini CLI / Antigravity: workspacePaths. Cursor: workspace_roots.
        let cwd = non_empty_str(map, "workdir").map(str::to_string).or_else(|| {
            ["workspacePaths", "workspace_roots"].iter().find_map(|k| {
                map.get(*k)?.as_array()?.first()?.as_str().filter(|s| !s.is_empty()).map(str::to_string)
            })
        });
        if let Some(cwd) = cwd {
            map.insert("cwd".into(), Value::String(cwd));
        }
    }
}

/// Agents whose tools are renamed to Claude Code's: those whose events come
/// from this repo's own plugins (Pi's extension, OpenCode's plugin). Other
/// agents keep their names, so their steps read as they always have.
const CANONICAL_AGENTS: &[&str] = &["pi", "orca", "opencode"];

/// Their tool names, lowercase, and Claude Code's for the same tool.
const TOOL_NAMES: &[(&str, &str)] = &[
    ("bash", "Bash"),
    ("powershell", "PowerShell"),
    ("read", "Read"),
    ("edit", "Edit"),
    ("write", "Write"),
    ("grep", "Grep"),
    ("glob", "Glob"),
    // Pi's find and ls, OpenCode's list: only the code view's Read phase reads them.
    ("find", "Glob"),
    ("ls", "LS"),
    ("list", "LS"),
];

/// Their argument names, and Claude Code's (Pi: `path`; OpenCode: `filePath`,
/// `oldString`, `newString`).
const INPUT_ALIASES: &[(&str, &str)] = &[
    ("path", "file_path"),
    ("filePath", "file_path"),
    ("oldString", "old_string"),
    ("newString", "new_string"),
];

/// An MCP server's own bash / edit / write / read / grep / find / ls (`mcp__<server>__<name>`),
/// for any agent, when its input has that tool's fields: a command for bash, a
/// file path for the others (and edits or content for edit and write), a
/// pattern for grep and find, a path for ls. An MCP
/// tool of the same name that means something else keeps its name.
fn mcp_tool<'a>(tool: &'a str, input: Option<&Value>) -> Option<&'a str> {
    let rest = tool.strip_prefix("mcp__")?;
    let (_, name) = rest.rsplit_once("__")?;
    let input = input?.as_object()?;
    let has = |k: &str| input.get(k).is_some_and(|v| !v.is_null());
    let file = has("path") || has("file_path") || has("filePath");
    let fits = match name {
        "bash" => has("command"),
        "edit" => file && (has("edits") || has("oldText") || has("old_string") || has("oldString")),
        "write" => file && has("content"),
        "read" => file,
        "grep" | "find" => has("pattern"),
        "ls" => has("path"),
        _ => false,
    };
    fits.then_some(name)
}

/// Pi's and OpenCode's tools under Claude Code's names and fields, so the live
/// diff and the code view read them as they read Claude Code's. Pi's `edit`
/// takes several replacements (`edits[].oldText / newText`): a MultiEdit.
pub fn canonical_tool(map: &mut Map<String, Value>) {
    let agent = map.get("coucou_agent").and_then(Value::as_str).unwrap_or_default();
    let tool = map.get("tool_name").and_then(Value::as_str).unwrap_or_default();
    let tool = if CANONICAL_AGENTS.contains(&agent) {
        tool.to_string()
    } else {
        match mcp_tool(tool, map.get("tool_input")) {
            Some(t) => t.to_string(),
            None => return,
        }
    };
    let Some(&(_, mut name)) = TOOL_NAMES.iter().find(|(from, _)| *from == tool) else { return };
    if let Some(Value::Object(input)) = map.get_mut("tool_input") {
        for (from, to) in INPUT_ALIASES {
            if let Some(v) = input.remove(*from) {
                input.entry((*to).to_string()).or_insert(v);
            }
        }
        if let Some(Value::Array(edits)) = input.get_mut("edits") {
            for edit in edits.iter_mut().filter_map(Value::as_object_mut) {
                for (from, to) in [("oldText", "old_string"), ("newText", "new_string")] {
                    if let Some(v) = edit.remove(from) {
                        edit.insert(to.into(), v);
                    }
                }
            }
            if name == "Edit" {
                name = "MultiEdit";
            }
        }
    }
    map.insert("tool_name".into(), Value::String(name.into()));
}

/// The event once the payload is known: a stop that reports an error (Cursor's
/// `status: "error"`) is a StopFailure.
pub fn refine(event: &str, map: &Map<String, Value>) -> String {
    if event == "Stop" && map.get("status").and_then(Value::as_str) == Some("error") {
        return "StopFailure".into();
    }
    event.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn every_agents_event_names_map_onto_claude_codes() {
        for (raw, canonical) in [
            ("BeforeTool", "PreToolUse"),
            ("BeforeToolSelection", "PreToolUse"),
            ("AfterTool", "PostToolUse"),
            ("AfterModel", "PostToolUse"),
            ("BeforeAgent", "UserPromptSubmit"),
            ("AfterAgent", "Stop"),
            ("startup", "SessionStart"),
            ("exit", "SessionEnd"),
            ("PreInvocation", "UserPromptSubmit"),
            ("PostInvocation", "PostToolUse"),
            ("pre_tool_use", "PreToolUse"),
            ("post_tool_use", "PostToolUse"),
            ("user_prompt_submit", "UserPromptSubmit"),
            ("session_start", "SessionStart"),
            ("session_end", "SessionEnd"),
            ("stop", "Stop"),
            ("sessionStart", "SessionStart"),
            ("userPromptSubmitted", "UserPromptSubmit"),
            ("agentStop", "Stop"),
            ("notification", "Notification"),
            ("preToolUse", "PreToolUse"),
            ("postToolUse", "PostToolUse"),
            ("permissionRequest", "PermissionRequest"),
            ("sessionEnd", "SessionEnd"),
            ("beforeSubmitPrompt", "UserPromptSubmit"),
            ("postToolUseFailure", "PostToolUseFailure"),
            ("subagentStart", "SubagentStart"),
            ("subagentStop", "SubagentStop"),
        ] {
            assert_eq!(event(raw), canonical, "{raw}");
        }
        // Claude Code's own names, and anything unknown, pass through.
        for same in ["PreToolUse", "PermissionRequest", "Stop", "Interrupt", "somethingNew"] {
            assert_eq!(event(same), same);
        }
    }

    #[test]
    fn antigravity_and_gemini_tool_calls_become_tool_name_and_input() {
        let mut m = obj(json!({
            "conversationId": "conv-1",
            "workspacePaths": ["C:\\Projects\\test"],
            "toolCall": { "name": "run_command", "args": { "CommandLine": "cargo check" } }
        }));
        fields(&mut m, &no_env);
        assert_eq!(m["tool_name"], "run_command");
        assert_eq!(m["tool_input"]["command"], "cargo check");
        assert_eq!(m["tool_input"]["CommandLine"], "cargo check");
        assert_eq!(m["session_id"], "conv-1");
        assert_eq!(m["cwd"], "C:\\Projects\\test");
    }

    #[test]
    fn copilot_tool_name_args_and_workdir_are_read() {
        let mut m = obj(json!({
            "toolName": "bash",
            "toolArgs": "{\"command\":\"npm test\"}",
            "sessionId": "s-9",
            "workdir": "/home/me/app"
        }));
        fields(&mut m, &no_env);
        assert_eq!(m["tool_name"], "bash");
        assert_eq!(m["tool_input"]["command"], "npm test");
        assert_eq!(m["session_id"], "s-9");
        assert_eq!(m["cwd"], "/home/me/app");

        let mut m = obj(json!({ "toolName": "edit", "toolArgs": { "path": "a.rs" } }));
        fields(&mut m, &no_env);
        assert_eq!(m["tool_input"]["path"], "a.rs");
    }

    #[test]
    fn cursor_conversation_and_workspace_become_session_and_cwd() {
        let mut m = obj(json!({ "conversation_id": "c-1", "workspace_roots": ["/w/proj"] }));
        fields(&mut m, &no_env);
        assert_eq!(m["session_id"], "c-1");
        assert_eq!(m["cwd"], "/w/proj");
    }

    #[test]
    fn what_the_payload_already_says_is_never_overwritten() {
        let mut m = obj(json!({
            "tool_name": "Bash", "tool_input": { "command": "ls" },
            "session_id": "keep", "cwd": "/keep",
            "toolName": "other", "toolArgs": { "command": "rm" },
            "conversationId": "no", "workdir": "/no"
        }));
        fields(&mut m, &|_| Some("env".into()));
        assert_eq!(m["tool_name"], "Bash");
        assert_eq!(m["tool_input"]["command"], "ls");
        assert_eq!(m["session_id"], "keep");
        assert_eq!(m["cwd"], "/keep");
    }

    #[test]
    fn gemini_session_comes_from_the_environment_last() {
        let mut m = obj(json!({}));
        fields(&mut m, &|k| (k == "GEMINI_SESSION_ID").then(|| "g-7".to_string()));
        assert_eq!(m["session_id"], "g-7");
    }

    #[test]
    fn odd_shapes_are_ignored_not_trusted() {
        let mut m = obj(json!({ "toolCall": "nope", "toolArgs": "not json", "workspacePaths": "x", "tool": 3 }));
        fields(&mut m, &no_env);
        assert!(!m.contains_key("tool_name"));
        assert!(!m.contains_key("tool_input"));
        assert!(!m.contains_key("cwd"));
    }

    #[test]
    fn a_stop_with_an_error_status_is_a_failure() {
        assert_eq!(refine("Stop", &obj(json!({ "status": "error" }))), "StopFailure");
        for status in ["completed", "aborted"] {
            assert_eq!(refine("Stop", &obj(json!({ "status": status }))), "Stop");
        }
        assert_eq!(refine("PreToolUse", &obj(json!({ "status": "error" }))), "PreToolUse");
    }

    #[test]
    fn pi_and_opencode_tools_take_claude_codes_names_and_fields() {
        // Pi: lowercase names, `path`, and its edits as oldText / newText.
        let mut m = obj(json!({ "coucou_agent": "pi", "tool_name": "edit",
            "tool_input": { "path": "src/a.ts", "edits": [{ "oldText": "a", "newText": "b" }] } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "MultiEdit");
        assert_eq!(m["tool_input"]["file_path"], "src/a.ts");
        assert_eq!(m["tool_input"]["edits"], json!([{ "old_string": "a", "new_string": "b" }]));

        let mut m = obj(json!({ "coucou_agent": "orca", "tool_name": "write",
            "tool_input": { "path": "/w/b.ts", "content": "x" } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "Write");
        assert_eq!(m["tool_input"], json!({ "file_path": "/w/b.ts", "content": "x" }));

        let mut m = obj(json!({ "coucou_agent": "pi", "tool_name": "bash", "tool_input": { "command": "ls" } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "Bash");

        // OpenCode: filePath, oldString, newString.
        let mut m = obj(json!({ "coucou_agent": "opencode", "tool_name": "edit",
            "tool_input": { "filePath": "/w/c.ts", "oldString": "a", "newString": "b" } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "Edit");
        assert_eq!(m["tool_input"], json!({ "file_path": "/w/c.ts", "old_string": "a", "new_string": "b" }));
        let mut m = obj(json!({ "coucou_agent": "opencode", "tool_name": "read", "tool_input": { "filePath": "/w/c.ts" } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "Read");
        assert_eq!(m["tool_input"]["file_path"], "/w/c.ts");
    }

    #[test]
    fn other_agents_and_unknown_tools_are_left_as_they_are() {
        // Copilot has an `edit` of its own, with other fields: untouched.
        for (agent, tool) in [("copilot", "edit"), ("gemini", "write_file"), ("pi", "my_tool")] {
            let before = obj(json!({ "coucou_agent": agent, "tool_name": tool, "tool_input": { "path": "a" } }));
            let mut m = before.clone();
            canonical_tool(&mut m);
            assert_eq!(m, before, "{agent} {tool}");
        }
        // Claude Code (no tag) keeps its own names.
        let before = obj(json!({ "tool_name": "Edit", "tool_input": { "file_path": "a" } }));
        let mut m = before.clone();
        canonical_tool(&mut m);
        assert_eq!(m, before);
    }

    #[test]
    fn mcp_file_and_shell_tools_read_as_claude_codes_own() {
        // Any session (here Claude Code, no tag): an MCP server's bash / edit /
        // write / read, with the fields of that tool.
        let mut m = obj(json!({ "tool_name": "mcp__custom-tools__bash", "tool_input": { "command": "ls" } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "Bash");
        let mut m = obj(json!({ "tool_name": "mcp__custom-tools__edit",
            "tool_input": { "path": "/w/a.ts", "edits": [{ "oldText": "a", "newText": "b" }] } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "MultiEdit");
        assert_eq!(m["tool_input"]["file_path"], "/w/a.ts");
        assert_eq!(m["tool_input"]["edits"], json!([{ "old_string": "a", "new_string": "b" }]));
        let mut m = obj(json!({ "tool_name": "mcp__fs__write", "tool_input": { "path": "/w/b.ts", "content": "x" } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "Write");
        let mut m = obj(json!({ "tool_name": "mcp__fs__read", "tool_input": { "path": "/w/b.ts" } }));
        canonical_tool(&mut m);
        assert_eq!(m["tool_name"], "Read");
    }

    #[test]
    fn mcp_tools_without_those_fields_keep_their_names() {
        for (tool, input) in [
            // Same name, other meaning: no file, no command.
            ("mcp__github__edit", json!({ "issue": 3, "body": "x" })),
            ("mcp__db__read", json!({ "table": "users" })),
            ("mcp__shell__bash", json!({ "script": "ls" })),
            // Not one of the four.
            ("mcp__custom-tools__mem_save", json!({ "title": "t" })),
            // Not an MCP name.
            ("custom__bash", json!({ "command": "ls" })),
        ] {
            let before = obj(json!({ "tool_name": tool, "tool_input": input }));
            let mut m = before.clone();
            canonical_tool(&mut m);
            assert_eq!(m, before, "{tool}");
        }
    }

    #[test]
    fn searching_and_listing_tools_count_as_reading() {
        // Pi's grep / find / ls, and the same MCP tools: the code view's Read phase.
        for (agent, tool, input, canonical) in [
            ("pi", "grep", json!({ "pattern": "x", "path": "." }), "Grep"),
            ("pi", "find", json!({ "pattern": "*.ts", "path": "." }), "Glob"),
            ("pi", "ls", json!({ "path": "." }), "LS"),
            ("", "mcp__custom-tools__grep", json!({ "pattern": "x", "path": "." }), "Grep"),
            ("", "mcp__custom-tools__find", json!({ "pattern": "*.ts" }), "Glob"),
            ("", "mcp__custom-tools__ls", json!({ "path": "." }), "LS"),
        ] {
            let mut m = obj(json!({ "tool_name": tool, "tool_input": input }));
            if !agent.is_empty() {
                m.insert("coucou_agent".into(), json!(agent));
            }
            canonical_tool(&mut m);
            assert_eq!(m["tool_name"], canonical, "{agent} {tool}");
        }
        // An MCP grep without a pattern is someone else's tool.
        let before = obj(json!({ "tool_name": "mcp__db__grep", "tool_input": { "table": "t" } }));
        let mut m = before.clone();
        canonical_tool(&mut m);
        assert_eq!(m, before);
    }
}
