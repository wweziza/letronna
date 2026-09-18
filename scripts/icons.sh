#!/usr/bin/env bash
# Pulls Phosphor Icons (Light weight) from npm into assets/icons.
# File names follow what gpui-component's IconName loads (Lucide-style names such as
# panel-left-close.svg), so the map below is "our name:phosphor name". Re-run to refresh.
set -euo pipefail
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
curl -sL -o "$tmp/p.tgz" https://registry.npmjs.org/@phosphor-icons/core/-/core-2.1.1.tgz
tar -xzf "$tmp/p.tgz" -C "$tmp"
src="$tmp/package/assets/light"
mkdir -p assets/icons
map="plus:plus close:x window-close:x window-minimize:minus window-maximize:square window-restore:browsers
minus:minus settings:gear arrow-up:arrow-up copy:copy panel-left-close:sidebar-simple panel-left-open:sidebar-simple
chevron-down:caret-down check:check loader-circle:circle-notch bot:robot mic:microphone chevrons-up-down:caret-up-down
inbox:tray send:paper-plane-tilt sparkles:sparkle message-square:chat-teardrop file:file file-text:file-text clock:clock
search:magnifying-glass pin:push-pin folder:folder image:image square-pen:note-pencil list-filter:funnel-simple"
for pair in $map; do
  cp "$src/${pair#*:}-light.svg" "assets/icons/${pair%%:*}.svg"
done
rm -rf "$tmp"
echo "icons updated: $(ls assets/icons | wc -l)"
