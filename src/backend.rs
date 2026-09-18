use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub model: String,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone)]
pub struct Connection {
    pub endpoint: String,
    pub model: String,
    pub key: String,
}

pub enum Event {
    Delta(String),
    FreeRemaining(u64),
    Finished(Result<(), String>),
}

pub fn validate(connection: &Connection) -> Result<String, String> {
    let url = reqwest::Url::parse(connection.endpoint.trim())
        .map_err(|_| "Enter a valid endpoint, such as http://localhost:11434/v1".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Endpoint must be an HTTP or HTTPS URL.".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use a base URL without credentials, query parameters, or fragments.".into());
    }
    if url.scheme() == "http"
        && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
    {
        return Err("Use HTTPS for remote providers, or HTTP on localhost.".into());
    }
    if connection.model.trim().is_empty() {
        return Err("Enter the model ID served by your provider in Connection settings.".into());
    }
    let base = connection.endpoint.trim().trim_end_matches('/');
    if connection.model.starts_with("aile-free/") {
        if base != "https://api.aile.sh/v1" {
            return Err("Aile Free models use the AILE connection: https://api.aile.sh/v1".into());
        }
        return Ok("https://chat.aile.sh/api/free/chat/completions".into());
    }
    Ok(if base.ends_with("/chat/completions") {
        base.into()
    } else {
        format!("{base}/chat/completions")
    })
}

fn body(connection: &Connection, messages: &[Message]) -> Value {
    // Bound context by characters and complete user turns; always retain the latest prompt.
    let mut start = messages.len();
    let mut chars = 0;
    while start > 0 && messages.len() - start < 20 {
        let size = messages[start - 1].content.chars().count();
        if chars + size > 24_000 {
            break;
        }
        chars += size;
        start -= 1;
    }
    while start < messages.len() && messages[start].role != "user" {
        start += 1;
    }
    let mut context = vec![
        json!({"role": "system", "content": "You are Letronna, a personal assistant. Be clear, concise, and honest. You can converse but have no tools or filesystem access in this app."}),
    ];
    context.extend(
        messages[start..]
            .iter()
            .map(|m| json!({"role": m.role, "content": m.content})),
    );
    json!({"model": connection.model.trim(), "messages": context, "stream": true, "max_tokens": 1024})
}

fn unwrap_response(value: &Value) -> Result<&Value, String> {
    if value.get("error").is_some() || value.get("success").is_some_and(|v| v != true) {
        let message = value
            .pointer("/error/message")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("The provider could not finish this response.");
        return Err(message.chars().take(400).collect());
    }
    Ok(if value.get("success") == Some(&Value::Bool(true)) {
        &value["data"]
    } else {
        value
    })
}

fn parse_event(data: &str) -> Result<Option<String>, String> {
    let value: Value = serde_json::from_str(data)
        .map_err(|_| "Provider returned an invalid stream event.".to_owned())?;
    Ok(unwrap_response(&value)?
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
        .map(str::to_owned))
}

pub fn data_dir() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("AGENT_DATA_DIR") {
        return path.into();
    }
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .unwrap_or_else(std::env::temp_dir)
        .join("Letronna")
}

pub fn list_models(base: &str, key: &str) -> Result<Vec<String>, String> {
    let base = base.trim().trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Cannot initialize connection.".to_owned())?;
    let mut request = client.get(format!("{base}/models"));
    if !key.trim().is_empty() {
        request = request.bearer_auth(key.trim());
    }
    let response = request
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| match e.status() {
            Some(s) if s.as_u16() == 401 || s.as_u16() == 403 => {
                "The gateway rejected the API key.".to_owned()
            }
            _ => "Cannot load the model list from this gateway.".to_owned(),
        })?;
    let value: Value = response
        .json()
        .map_err(|_| "Invalid model list.".to_owned())?;
    let models = unwrap_response(&value)?["data"]
        .as_array()
        .ok_or("Invalid model list.")?
        .iter()
        .filter_map(|m| m["id"].as_str().or_else(|| m["key"].as_str()))
        .map(str::to_owned)
        .collect();
    Ok(models)
}

pub fn free_models() -> Result<Vec<String>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Cannot initialize connection.".to_owned())?;
    let response = client
        .get("https://chat.aile.sh/api/free/models")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| "Cannot load the AILE free model catalog.".to_owned())?;
    let value: Value = response
        .json()
        .map_err(|_| "Invalid model catalog.".to_owned())?;
    let models = value["data"]
        .as_array()
        .ok_or("Invalid model catalog.")?
        .iter()
        .filter_map(|m| m["key"].as_str())
        .filter(|id| id.starts_with("aile-free/"))
        .map(str::to_owned)
        .collect();
    Ok(models)
}

fn free_session(client: &reqwest::blocking::Client) -> Result<(String, u64), String> {
    let path = data_dir().join("aile-session.txt");
    let stored = match std::fs::read_to_string(&path) {
        Ok(value) => value,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err("Cannot read the saved AILE session.".into()),
    };
    let mut request = client.get("https://chat.aile.sh/api/free/session");
    if !stored.is_empty() {
        request = request.header(reqwest::header::COOKIE, stored.trim());
    }
    let response = request
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| "Cannot open an AILE free session.".to_owned())?;
    let cookie = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|v| v.split(';').next())
        .find(|v| v.starts_with("aile_private_guest="))
        .map(str::to_owned)
        .unwrap_or(stored);
    let value: Value = response
        .json()
        .map_err(|_| "Invalid AILE session response.".to_owned())?;
    let remaining = value["remaining"]
        .as_u64()
        .ok_or("AILE returned no guest allowance.")?;
    if cookie.is_empty() {
        return Err("AILE did not issue a guest session.".into());
    }
    std::fs::create_dir_all(data_dir())
        .and_then(|_| std::fs::write(path, &cookie))
        .map_err(|_| {
            "Cannot save the AILE session. Check your data-folder permissions.".to_owned()
        })?;
    if remaining == 0 {
        return Err("Your AILE guest allowance is used up. Sign in at chat.aile.sh to continue there; native account sign-in is not implemented yet. No paid fallback was used.".into());
    }
    Ok((cookie, remaining))
}

pub fn start(connection: Connection, messages: Vec<Message>) -> async_channel::Receiver<Event> {
    let (sender, receiver) = async_channel::bounded(64);
    std::thread::spawn(move || {
        let result = stream(&connection, &messages, &sender);
        let _ = sender.send_blocking(Event::Finished(result));
    });
    receiver
}

fn stream(
    connection: &Connection,
    messages: &[Message],
    sender: &async_channel::Sender<Event>,
) -> Result<(), String> {
    let endpoint = validate(connection)?;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize the HTTP client.".to_owned())?;
    let mut request = client.post(endpoint).json(&body(connection, messages));
    let free_remaining = if connection.model.starts_with("aile-free/") {
        let (cookie, remaining) = free_session(&client)?;
        request = request.header(reqwest::header::COOKIE, cookie);
        let _ = sender.send_blocking(Event::FreeRemaining(remaining));
        Some(remaining)
    } else {
        if !connection.key.trim().is_empty() {
            request = request.bearer_auth(connection.key.trim());
        }
        None
    };
    let response = request.send().map_err(|_| "Cannot reach the model. Check that your server is running and the endpoint is correct (120-second request timeout).".to_owned())?;
    let status = response.status();
    if !status.is_success() {
        let detail = response
            .json::<Value>()
            .ok()
            .and_then(|v| unwrap_response(&v).err());
        return Err(detail.unwrap_or_else(|| {
            format!(
                "Provider returned HTTP {}. Check connection settings and quota.",
                status.as_u16()
            )
        }));
    }
    if response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.contains("application/json"))
    {
        let value: Value = response
            .json()
            .map_err(|_| "Invalid model response.".to_owned())?;
        let content = unwrap_response(&value)?
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or("Model returned no text.")?
            .to_owned();
        sender
            .send_blocking(Event::Delta(content))
            .map_err(|_| "Chat closed.".to_owned())?;
        if let Some(n) = free_remaining {
            let _ = sender.send_blocking(Event::FreeRemaining(n.saturating_sub(1)));
        }
        return Ok(());
    }
    let mut received = false;
    let mut complete = false;
    for line in BufReader::new(response).lines() {
        let line =
            line.map_err(|_| "Connection interrupted before the response completed.".to_owned())?;
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" {
            complete = true;
            break;
        }
        if let Some(text) = parse_event(data)? {
            received |= !text.is_empty();
            sender
                .send_blocking(Event::Delta(text))
                .map_err(|_| "Chat closed.".to_owned())?;
        }
    }
    if !complete {
        return Err(
            "The provider closed the stream early or does not support SSE chat completions.".into(),
        );
    }
    if !received {
        return Err("The model returned no text. Check model compatibility.".into());
    }
    if let Some(n) = free_remaining {
        let _ = sender.send_blocking(Event::FreeRemaining(n.saturating_sub(1)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connection() -> Connection {
        Connection {
            endpoint: "http://localhost:11434/v1/".into(),
            model: "test-model".into(),
            key: String::new(),
        }
    }
    #[test]
    fn endpoint_validation() {
        assert_eq!(
            validate(&connection()).unwrap(),
            "http://localhost:11434/v1/chat/completions"
        );
        let mut c = connection();
        c.model.clear();
        assert!(validate(&c).is_err());
        c.model = "test".into();
        c.endpoint = "http://example.com/v1".into();
        assert!(validate(&c).is_err());
    }
    #[test]
    fn parses_text_and_provider_errors() {
        assert_eq!(
            parse_event(r#"{"choices":[{"delta":{"content":"hello"}}]}"#).unwrap(),
            Some("hello".into())
        );
        assert!(parse_event(r#"{"error":{"message":"bad"}}"#).is_err());
        assert_eq!(
            parse_event(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#).unwrap(),
            None
        );
    }
    #[test]
    fn aile_free_contract() {
        assert_eq!(
            parse_event(
                r#"{"success":true,"data":{"choices":[{"delta":{"content":"AILE reply"}}]}}"#
            )
            .unwrap(),
            Some("AILE reply".into())
        );
        assert!(parse_event(r#"{"success":false,"message":"Quota exhausted"}"#).is_err());
        let c = Connection {
            endpoint: "https://api.aile.sh/v1".into(),
            model: "aile-free/gpt-oss-20b".into(),
            key: String::new(),
        };
        assert_eq!(
            validate(&c).unwrap(),
            "https://chat.aile.sh/api/free/chat/completions"
        );
    }
    #[test]
    #[ignore = "Uses one real AILE guest prompt; run explicitly"]
    fn live_aile_free_chat() {
        let rx = start(
            Connection {
                endpoint: "https://api.aile.sh/v1".into(),
                model: "aile-free/gpt-oss-20b".into(),
                key: String::new(),
            },
            vec![Message {
                role: "user".into(),
                content: "Reply with exactly: Native chat works.".into(),
                model: String::new(),
            }],
        );
        let mut text = String::new();
        loop {
            match rx.recv_blocking().unwrap() {
                Event::Delta(chunk) => text.push_str(&chunk),
                Event::FreeRemaining(_) => {}
                Event::Finished(result) => {
                    result.unwrap();
                    break;
                }
            }
        }
        assert!(!text.trim().is_empty());
        println!("AILE live reply: {text}");
    }
    #[test]
    fn context_keeps_complete_recent_turns() {
        let messages: Vec<_> = (0..30)
            .map(|i| Message {
                role: if i % 2 == 0 { "user" } else { "assistant" }.into(),
                content: format!("message {i}"),
                model: String::new(),
            })
            .collect();
        let payload = body(&connection(), &messages);
        let context = payload["messages"].as_array().unwrap();
        assert_eq!(context.len(), 21);
        assert_eq!(context[1]["role"], "user");
        assert_eq!(context.last().unwrap()["content"], "message 29");
    }
    #[test]
    fn streams_from_local_http_server() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1", server.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut block = [0; 4096];
            loop {
                let n = socket.read(&mut block).unwrap();
                bytes.extend_from_slice(&block[..n]);
                if let Some(header_end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..header_end]).to_lowercase();
                    let len: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= header_end + 4 + len {
                        let payload: Value =
                            serde_json::from_slice(&bytes[header_end + 4..]).unwrap();
                        assert_eq!(payload["stream"], true);
                        break;
                    }
                }
                assert_ne!(n, 0);
            }
            let body =
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\ndata: [DONE]\n\n";
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let mut c = connection();
        c.endpoint = endpoint;
        let rx = start(
            c,
            vec![Message {
                role: "user".into(),
                content: "Hi".into(),
                model: String::new(),
            }],
        );
        assert!(matches!(rx.recv_blocking().unwrap(), Event::Delta(s) if s == "Hello"));
        assert!(matches!(
            rx.recv_blocking().unwrap(),
            Event::Finished(Ok(()))
        ));
        worker.join().unwrap();
    }
}
