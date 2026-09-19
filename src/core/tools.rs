//! Tools the model may call. Each one is a `Tool`; the registry below is what
//! gets advertised to the provider and dispatched on `tool_calls`.
//!
//! Phase 1 ships read-only tools, which run without asking. Anything that
//! writes or executes goes through an approval card (phase 2).

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
}

const MAX_OUTPUT: usize = 24_000;
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", ".next", "dist", "build"];

pub fn all() -> Vec<Box<dyn Tool>> {
    vec![Box::new(ReadFile), Box::new(ListDir), Box::new(Search)]
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
    let tool = all()
        .into_iter()
        .find(|t| t.name() == name)
        .ok_or_else(|| format!("Unknown tool `{name}`."))?;
    let args: Value = if args.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(args).map_err(|e| format!("Invalid arguments: {e}"))?
    };
    let mut out = tool.run(&args, ctx)?;
    if out.len() > MAX_OUTPUT {
        let cut = out.floor_char_boundary(MAX_OUTPUT);
        out.truncate(cut);
        out.push_str("\n… [truncated]");
    }
    Ok(out)
}

/// The argument the UI shows next to a tool name.
pub fn summary(name: &str, args: &str) -> String {
    let v: Value = serde_json::from_str(args).unwrap_or(Value::Null);
    match name {
        "search" => v["query"]
            .as_str()
            .map(|q| format!("\"{q}\""))
            .unwrap_or_default(),
        _ => v["path"].as_str().unwrap_or(".").to_owned(),
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
        std::fs::remove_dir_all(dir).unwrap();
    }
}
