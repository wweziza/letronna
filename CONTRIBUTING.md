# Contributing

Thanks for looking. Letronna is early, so the bar is simple: keep it small, keep it native.

## Ground rules

- No web views, no Electron, no JavaScript. Everything renders through GPUI.
- Prefer what gpui-component already ships over a new widget.
- Use theme tokens (`cx.theme().…`), not hard-coded colours.
- Icons come from Phosphor Light via `scripts/icons.sh`. Add the name to the map there, do not hand-draw SVGs.
- One feature per pull request. Describe what changed and why in a few lines.

## Before you push

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --release
cargo build --release
```

`scripts/verify-ui.ps1 -Action capture` screenshots the running release build into `artifacts/` if you want to attach a picture.

## Where things live

- `src/main.rs` the whole UI. Views are small `fn` blocks on `Chat`.
- `src/backend.rs` HTTP, validation, streaming, model lists. Tests live here.
- `assets/icons` generated, do not edit by hand.

## Reporting bugs

Open an issue with the gateway you used, the model, and what you expected. Never paste an API key.
