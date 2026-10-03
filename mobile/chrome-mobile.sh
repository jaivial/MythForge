#!/usr/bin/env bash
# Chrome wrapper for the mobile e2e pass: MINITUI_CHROME=mobile/chrome-mobile.sh
# Starts the real Chrome with a touch/coarse-pointer/no-hover phone profile and a
# sidecar that pins every page to a 390x844 mobile viewport over DevTools
# (headless Chrome refuses windows narrower than 500px, so a flag alone is not enough).
HERE="$(cd "$(dirname "$0")" && pwd)"
CHROME="${MOBILE_CHROME_BIN:-$(ls -d "$HOME"/.agent-browser/browsers/chrome-*/chrome 2>/dev/null | tail -1)}"
if [ -z "$CHROME" ] || [ ! -x "$CHROME" ]; then
  echo "chrome-mobile.sh: no Chrome found (set MOBILE_CHROME_BIN)" >&2
  exit 127
fi
python3 -c 'import websocket' 2>/dev/null || {
  echo "chrome-mobile.sh: python3 websocket-client missing (pip install websocket-client); refusing to run at desktop size" >&2
  exit 127
}
PROFILE=""
for a in "$@"; do case "$a" in --user-data-dir=*) PROFILE="${a#--user-data-dir=}";; esac; done
[ -n "$PROFILE" ] && python3 "$HERE/phone-sidecar.py" "$PROFILE" >/dev/null 2>&1 &
exec "$CHROME" "$@" \
  --touch-events=enabled \
  --blink-settings=primaryPointerType=2,availablePointerTypes=2,primaryHoverType=1,availableHoverTypes=1 \
  --user-agent="Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1"
