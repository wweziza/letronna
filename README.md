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

On first launch it registers itself: the executable is copied to `%LOCALAPPDATA%\Programs\Letronna`, a Start Menu entry is added so it shows up in Windows search, and an entry appears in Apps & features. Newer builds re-register on launch. Uninstall from Apps & features, or run `Letronna.exe --uninstall`.

If Discord is running, Letronna shows a rich presence (Idle, or Chatting with the model). Set `LETRONNA_DISCORD_APP_ID` to your own Discord application id, and upload a `letronna` art asset to it, for the icon to appear.

Closing the window keeps Letronna in the notification area. Left-click the tray icon to bring it back; right-click for Check for updates and Quit.

First launch uses the AILE Free gateway, which needs no key and gives a few guest replies. Open **Settings** at the bottom of the sidebar to pick another gateway, paste an API key, and reload the model list.

## Gateways

| Gateway     | Base URL                          | Key      |
|-------------|-----------------------------------|----------|
| AILE Free   | `https://api.aile.sh/v1`          | none     |
| AILE        | `https://api.aile.sh/v1`          | required |

AILE Free talks to the guest chat API and lists the free catalog (`aile-free/…` ids). AILE talks to the marketplace API with your key and lists its own catalog. The two catalogs are separate, so a model shown on one gateway is not necessarily served on the other.
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
target
elease\letronna.exe   # registers itself on first run
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
src/main.rs         entry point: wires plugins, tray, window
src/app.rs          Chat state, sessions, sending, render root
src/prelude.rs      shared imports
src/theme.rs        fonts and palette
src/assets.rs       embedded icons and the AppIcon enum

src/core/           domain layer (no UI, no OS)
  chat.rs           HTTP: validation, streaming, model lists, reasoning, usage
  gateway.rs        gateway presets
  store.rs          on-disk sessions and keys

src/ui/             view surfaces (impl Chat blocks)
  sidebar.rs composer.rs messages.rs settings.rs sessions.rs chrome.rs

src/plugins/        optional integrations behind a Plugin trait + event bus
  mod.rs            Plugin trait, AppEvent, registry, emit()
  discord.rs        Discord rich presence

src/platform/       Windows-only OS integration (cfg(windows))
  install.rs        first-run registration and --uninstall
  tray.rs           notification-area icon and menu
  window.rs         Win32 window helpers

assets/             icon, avatar, SVG icons
scripts/            icon sync, window capture for UI checks
```

Adding an integration (a new plugin) is one file in `src/plugins` implementing
`Plugin`, plus one line in `main`. The app only emits `AppEvent`s; it never
calls a plugin directly.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Small, focused pull requests are welcome.

## License

MIT, © 2026 Gadlus Engineering. GPUI and gpui-component are Apache-2.0. Phosphor Icons are MIT.

Developed by the Gadlus Engineering team.
