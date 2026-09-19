//! Tools the model may call. Each one is a `Tool`; the registry below is what
//! gets advertised to the provider and dispatched on `tool_calls`.
//!
//! Read-only tools run without asking. Tools that change the workspace or
//! run a command return `needs_approval`, and the chat shows a card first.
//! `ask_user` is answered by the user, never run here.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Where a session is allowed to look.
#[derive(Clone, Default)]
pub struct ToolContext {
    /// The folder attached to the session. Relative paths resolve here.
    pub workspace: Option<PathBuf>,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// JSON schema for the arguments object.
    fn parameters(&self) -> Value;
    fn run(&self, args: &Value, ctx: &ToolContext) -> Result<String, String>;
    /// Whether the user must approve each call before it runs.
    fn needs_approval(&self) -> bool {
        false
    }
    /// What the approval card shows: a diff, the file body, the command line.
    fn preview(&self, args: &Value, ctx: &ToolContext) -> String {
        let _ = ctx;
        args.to_string()
    }
}

pub const ASK_USER: &str = "ask_user";
/// The tool result the model gets when the user denies a call.
pub const DECLINED: &str = "The user declined this action. Ask before trying a different approach.";
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

const MAX_OUTPUT: usize = 24_000;
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", ".next", "dist", "build"];

pub fn all() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ReadFile),
        Box::new(ListDir),
        Box::new(Search),
        Box::new(WriteFile),
        Box::new(EditFile),
        Box::new(RunCommand),
        Box::new(AskUser),
    ]
}

pub fn find(name: &str) -> Option<Box<dyn Tool>> {
    all().into_iter().find(|t| t.name() == name)
}

pub fn parse_args(args: &str) -> Result<Value, String> {
    if args.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(args).map_err(|e| format!("Invalid arguments: {e}"))
    }
}

pub fn needs_approval(name: &str) -> bool {
    find(name).is_some_and(|t| t.needs_approval())
}

pub fn preview(name: &str, args: &str, ctx: &ToolContext) -> String {
    match (find(name), parse_args(args)) {
        (Some(t), Ok(v)) => t.preview(&v, ctx),
        _ => args.to_owned(),
    }
}

/// The `tools` array for a chat completion request.
pub fn specs() -> Vec<Value> {
    all()
        .iter()
        .map(|t| {
            json!({"type": "function", "function": {
                "name": t.name(),
                "description": t.description(),
                "parameters": t.parameters(),
            }})
        })
        .collect()
}

pub fn run(name: &str, args: &str, ctx: &ToolContext) -> Result<String, String> {
    let tool = find(name).ok_or_else(|| format!("Unknown tool `{name}`."))?;
    let args = parse_args(args)?;
    let mut out = tool.run(&args, ctx)?;
    if out.len() > MAX_OUTPUT {
        let cut = out.floor_char_boundary(MAX_OUTPUT);
        out.truncate(cut);
        out.push_str("\n… [truncated]");
    }
    Ok(out)
}

/// The first `max` lines, with a note about the rest. Keeps cards and rows
/// from needing their own scroll area inside the chat.
pub fn clamp_lines(text: &str, max: usize) -> String {
    let total = text.lines().count();
    if total <= max {
        return text.to_owned();
    }
    let mut out: String = text.lines().take(max).collect::<Vec<_>>().join(
        "
",
    );
    out.push_str(&format!(
        "
… {} more lines",
        total - max
    ));
    out
}

/// The argument the UI shows next to a tool name.
pub fn summary(name: &str, args: &str) -> String {
    let v: Value = serde_json::from_str(args).unwrap_or(Value::Null);
    match name {
        "search" => v["query"]
            .as_str()
            .map(|q| format!("\"{q}\""))
            .unwrap_or_default(),
        "run_command" => v["command"].as_str().unwrap_or_default().to_owned(),
        ASK_USER => v["question"].as_str().unwrap_or_default().to_owned(),
        _ => v["path"].as_str().unwrap_or(".").to_owned(),
    }
}

/// Where a write may land: inside the workspace only. The file need not
/// exist yet, so the check is on the nearest existing ancestor.
fn resolve_write(raw: &str, ctx: &ToolContext) -> Result<PathBuf, String> {
    let workspace = ctx
        .workspace
        .as_ref()
        .ok_or("No project folder is attached. Ask the user to attach one first.")?;
    let root = std::fs::canonicalize(workspace).map_err(|_| "The project folder is missing.")?;
    let target = workspace.join(raw);
    let mut probe = target.clone();
    while !probe.exists() {
        probe = probe.parent().map(Path::to_path_buf).ok_or("Bad path.")?;
    }
    let existing = std::fs::canonicalize(&probe).map_err(|e| e.to_string())?;
    if !existing.starts_with(&root) {
        return Err(format!("`{raw}` is outside the project folder."));
    }
    Ok(target)
}

fn run_shell(command: &str, cwd: Option<&Path>) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-Command", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start the shell: {e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > COMMAND_TIMEOUT {
            let _ = child.kill();
            return Err(format!(
                "Timed out after {}s. Long-running commands are not supported yet.",
                COMMAND_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let mut text = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    if !err.trim().is_empty() {
        text.push_str("\n[stderr]\n");
        text.push_str(&err);
    }
    if !status.success() {
        text.push_str(&format!("\n[exit code {}]", status.code().unwrap_or(-1)));
    }
    Ok(text)
}

struct WriteFile;
impl Tool for WriteFile {
    fn name(&self) -> &'static str {
        "write_file"
    }
    fn description(&self) -> &'static str {
        "Create or overwrite a text file in the project folder. Parent folders are created."
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {
            "path": {"type": "string"},
            "content": {"type": "string"}
        }, "required": ["path", "content"]})
    }
    fn needs_approval(&self) -> bool {
        true
    }
    fn preview(&self, args: &Value, _: &ToolContext) -> String {
        args["content"].as_str().unwrap_or_default().to_owned()
    }
    fn run(&self, args: &Value, ctx: &ToolContext) -> Result<String, String> {
        let path = resolve_write(path_arg(args), ctx)?;
        let content = args["content"].as_str().ok_or("`content` is required.")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(format!("Wrote {} bytes.", content.len()))
    }
}

struct EditFile;
impl Tool for EditFile {
    fn name(&self) -> &'static str {
        "edit_file"
    }
    fn description(&self) -> &'static str {
        "Replace one exact occurrence of `old` with `new` in a file. `old` must match exactly once."
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {
            "path": {"type": "string"},
            "old": {"type": "string"},
            "new": {"type": "string"}
        }, "required": ["path", "old", "new"]})
    }
    fn needs_approval(&self) -> bool {
        true
    }
    fn preview(&self, args: &Value, _: &ToolContext) -> String {
        let old = args["old"].as_str().unwrap_or_default();
        let new = args["new"].as_str().unwrap_or_default();
        let mut out = String::new();
        for line in old.lines() {
            out.push_str("- ");
            out.push_str(line);
            out.push('\n');
        }
        for line in new.lines() {
            out.push_str("+ ");
            out.push_str(line);
            out.push('\n');
        }
        out
    }
    fn run(&self, args: &Value, ctx: &ToolContext) -> Result<String, String> {
        let path = resolve_write(path_arg(args), ctx)?;
        let old = args["old"].as_str().ok_or("`old` is required.")?;
        let new = args["new"].as_str().ok_or("`new` is required.")?;
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match text.matches(old).count() {
            0 => return Err("`old` was not found in the file.".into()),
            1 => {}
            n => return Err(format!("`old` matches {n} times; include more context.")),
        }
        std::fs::write(&path, text.replacen(old, new, 1)).map_err(|e| e.to_string())?;
        Ok("Edited.".into())
    }
}

struct RunCommand;
impl Tool for RunCommand {
    fn name(&self) -> &'static str {
        "run_command"
    }
    fn description(&self) -> &'static str {
        if cfg!(windows) {
            "Run a PowerShell command in the project folder and return its output. 2 minute limit; do not start servers or watchers."
        } else {
            "Run a shell (sh) command in the project folder and return its output. 2 minute limit; do not start servers or watchers."
        }
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {
            "command": {"type": "string"}
        }, "required": ["command"]})
    }
    fn needs_approval(&self) -> bool {
        true
    }
    fn preview(&self, args: &Value, _: &ToolContext) -> String {
        args["command"].as_str().unwrap_or_default().to_owned()
    }
    fn run(&self, args: &Value, ctx: &ToolContext) -> Result<String, String> {
        let command = args["command"].as_str().ok_or("`command` is required.")?;
        let cwd = ctx
            .workspace
            .as_deref()
            .ok_or("No project folder is attached. Ask the user to attach one first.")?;
        run_shell(command, Some(cwd))
    }
}

/// Answered in the chat, not here.
struct AskUser;
impl Tool for AskUser {
    fn name(&self) -> &'static str {
        ASK_USER
    }
    fn description(&self) -> &'static str {
        "Ask the user a question and wait for the answer. Use it when the task is ambiguous or needs a decision, such as a project name or framework."
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {
            "question": {"type": "string"},
            "options": {"type": "array", "items": {"type": "string"}, "description": "Optional choices to offer."}
        }, "required": ["question"]})
    }
    fn needs_approval(&self) -> bool {
        true
    }
    fn run(&self, _: &Value, _: &ToolContext) -> Result<String, String> {
        Err("ask_user is answered by the user.".into())
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Resolve a user-supplied path and refuse anything outside the workspace or
/// the home folder. Canonicalising also defeats `..` and symlink escapes.
fn resolve(raw: &str, ctx: &ToolContext) -> Result<PathBuf, String> {
    let base = ctx.workspace.clone().or_else(home).unwrap_or_default();
    let joined = base.join(raw);
    let path = std::fs::canonicalize(&joined).map_err(|_| format!("`{raw}` does not exist."))?;
    let allowed = [ctx.workspace.clone(), home()]
        .into_iter()
        .flatten()
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .any(|root| path.starts_with(root));
    if !allowed {
        return Err(format!("`{raw}` is outside the workspace and home folder."));
    }
    Ok(path)
}

fn path_arg(args: &Value) -> &str {
    args["path"].as_str().unwrap_or(".")
}

struct ReadFile;
impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "read_file"
    }
    fn description(&self) -> &'static str {
        "Read a text file. Paths are relative to the workspace."
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {
            "path": {"type": "string"}
        }, "required": ["path"]})
    }
    fn run(&self, args: &Value, ctx: &ToolContext) -> Result<String, String> {
        let path = resolve(path_arg(args), ctx)?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        String::from_utf8(bytes).map_err(|_| "Not a text file.".into())
    }
}

struct ListDir;
impl Tool for ListDir {
    fn name(&self) -> &'static str {
        "list_dir"
    }
    fn description(&self) -> &'static str {
        "List the entries of a folder. Folders end with a slash."
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {
            "path": {"type": "string", "description": "Defaults to the workspace root."}
        }})
    }
    fn run(&self, args: &Value, ctx: &ToolContext) -> Result<String, String> {
        let path = resolve(path_arg(args), ctx)?;
        let mut names: Vec<String> = std::fs::read_dir(&path)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.path().is_dir() { name + "/" } else { name }
            })
            .collect();
        names.sort();
        Ok(names.join("\n"))
    }
}

struct Search;
impl Tool for Search {
    fn name(&self) -> &'static str {
        "search"
    }
    fn description(&self) -> &'static str {
        "Case-insensitive text search across files under a folder. Returns path:line: text."
    }
    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {
            "query": {"type": "string"},
            "path": {"type": "string", "description": "Folder to search. Defaults to the workspace root."}
        }, "required": ["query"]})
    }
    fn run(&self, args: &Value, ctx: &ToolContext) -> Result<String, String> {
        let query = args["query"].as_str().unwrap_or("").to_lowercase();
        if query.is_empty() {
            return Err("Query is empty.".into());
        }
        let root = resolve(path_arg(args), ctx)?;
        let mut hits = Vec::new();
        walk(&root, &mut |file| {
            let Ok(text) = std::fs::read_to_string(file) else {
                return true;
            };
            for (n, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(&query) {
                    let rel = file.strip_prefix(&root).unwrap_or(file).display();
                    hits.push(format!("{rel}:{}: {}", n + 1, line.trim()));
                    if hits.len() >= 200 {
                        return false;
                    }
                }
            }
            true
        });
        if hits.is_empty() {
            Ok("No matches.".into())
        } else {
            Ok(hits.join("\n"))
        }
    }
}

/// Depth-first walk skipping build and VCS folders. The callback returns false to stop.
fn walk(dir: &Path, f: &mut dyn FnMut(&Path) -> bool) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return true;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if SKIP_DIRS.iter().any(|s| name == *s) {
                continue;
            }
            if !walk(&path, f) {
                return false;
            }
        } else if !f(&path) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_lists_searches_within_workspace() {
        let dir = std::env::temp_dir().join(format!("letronna-tools-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.txt"), "hello\nWorld").unwrap();
        let ctx = ToolContext {
            workspace: Some(dir.clone()),
        };
        assert_eq!(
            run("read_file", r#"{"path":"src/a.txt"}"#, &ctx).unwrap(),
            "hello\nWorld"
        );
        assert_eq!(run("list_dir", "{}", &ctx).unwrap(), "src/");
        assert_eq!(
            run("search", r#"{"query":"world"}"#, &ctx).unwrap(),
            "src\\a.txt:2: World".replace('\\', std::path::MAIN_SEPARATOR_STR)
        );
        // The filesystem root is outside both the workspace and home.
        assert!(run("list_dir", r#"{"path":"/"}"#, &ctx).is_err());
        run(
            "write_file",
            r#"{"path":"new/b.txt","content":"one two"}"#,
            &ctx,
        )
        .unwrap();
        run(
            "edit_file",
            r#"{"path":"new/b.txt","old":"two","new":"three"}"#,
            &ctx,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("new/b.txt")).unwrap(),
            "one three"
        );
        assert!(
            run(
                "edit_file",
                r#"{"path":"new/b.txt","old":"zzz","new":""}"#,
                &ctx
            )
            .is_err()
        );
        assert!(
            run(
                "write_file",
                r#"{"path":"../escape.txt","content":""}"#,
                &ctx
            )
            .is_err()
        );
        let no_ws = ToolContext::default();
        assert!(run("write_file", r#"{"path":"x","content":""}"#, &no_ws).is_err());
        let out = run("run_command", r#"{"command":"echo hi"}"#, &ctx).unwrap();
        assert!(out.trim_start().starts_with("hi"), "{out}");
        assert!(needs_approval("run_command") && !needs_approval("read_file"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
