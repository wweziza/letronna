<p align="center">
  <img src="assets/letronna-front.png" width="320" alt="Letronna">
</p>

<h1 align="center">Letronna</h1>

<p align="center">
  A small native desktop agent. Rust, GPU rendered, no browser inside.<br>
  <b>Alpha.</b> Expect rough edges and breaking changes.
</p>

## What it is

Letronna is a chat-first desktop app built on [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) and [gpui-component](https://github.com/longbridge/gpui-component). It talks to any OpenAI-compatible endpoint through a gateway you pick, streams replies, and keeps sessions on your machine.

- Sessions sidebar with search, pinning and date groups
- Markdown replies, streaming, multi-line composer
- Gateways: AILE Free, AILE, OpenAI, OpenRouter, Ollama, or a custom base URL
- Searchable model picker fed by the gateway's model list
- Single ~24 MB executable, no runtime, no Electron

## Run it

Download or build `letronna.exe` and double-click it. Windows only for now.

First launch uses the AILE Free gateway, which needs no key and gives a few guest replies. Open **Settings** at the bottom of the sidebar to pick another gateway, paste an API key, and reload the model list.

## Gateways

| Gateway     | Base URL                          | Key      |
|-------------|-----------------------------------|----------|
| AILE Free   | `https://api.aile.sh/v1`          | none     |
| AILE        | `https://api.aile.sh/v1`          | required |
| OpenAI      | `https://api.openai.com/v1`       | required |
| OpenRouter  | `https://openrouter.ai/api/v1`    | required |
| Ollama      | `http://localhost:11434/v1`       | none     |
| Custom      | anything OpenAI-compatible        | optional |

Model lists come from `GET {base}/models`. Streaming uses SSE chat completions.

## Local data

`%LOCALAPPDATA%\Letronna\`

- `chats.json` sessions and the selected gateway, endpoint and model
- `keys.json` API keys per gateway, plain text on your disk only
- `aile-session.txt` the AILE guest cookie

Chat text is sent to the gateway you selected and nowhere else.

## Build

Requirements: Rust, Visual Studio C++ build tools, and a Windows SDK with `fxc.exe`. Set `GPUI_FXC_PATH` in `.cargo/config.toml` to your SDK's shader compiler.

```powershell
cargo build --release
scripts\install.ps1      # copies to %LOCALAPPDATA%\Programs\Letronna and adds a Start Menu entry
```

Checks:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
```

Icons are Phosphor Light. `scripts/icons.sh` re-syncs them from npm.

## Layout

```
src/main.rs         entry point and window setup
src/app.rs          Chat state, sessions, sending, model loading, render root
src/store.rs        on-disk sessions and keys
src/gateway.rs      gateway presets
src/theme.rs        fonts and palette
src/assets.rs       embedded icons and the AppIcon enum
src/backend.rs      HTTP: validation, streaming, model lists, local paths
src/ui/sidebar.rs   nav, search, session list
src/ui/composer.rs  input row, attach menu, model picker
src/ui/messages.rs  message rows, empty state, placeholder pages
src/ui/settings.rs  gateway dialog
src/ui/chrome.rs    title bar and footer
assets/             app icon, avatar, SVG icons
scripts/            install, icon sync, window capture for UI checks
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Small, focused pull requests are welcome.

## License

MIT, © 2026 Gadlus Engineering. GPUI and gpui-component are Apache-2.0. Phosphor Icons are MIT.

Developed by the Gadlus Engineering team.
