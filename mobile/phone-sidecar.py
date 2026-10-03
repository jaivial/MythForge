#!/usr/bin/env python3
"""Hold a DevTools session on every page of a Chrome profile and pin it to a phone
viewport (Emulation.setDeviceMetricsOverride). Runs in Chrome's process group, so the
e2e runner kills it together with Chrome. Usage: phone-sidecar.py <user-data-dir>"""
import json, os, sys, threading, time, urllib.request
import websocket

W, H = int(os.environ.get("MOBILE_W", 390)), int(os.environ.get("MOBILE_H", 844))
profile = sys.argv[1]
port_file = os.path.join(profile, "DevToolsActivePort")
for _ in range(400):
    if os.path.exists(port_file) and open(port_file).read().strip():
        break
    time.sleep(0.05)
port = open(port_file).read().split()[0]

def pin(ws_url):
    ws = websocket.create_connection(ws_url, timeout=None, suppress_origin=True)
    for i, (m, p) in enumerate([
        ("Emulation.setDeviceMetricsOverride", {"width": W, "height": H, "deviceScaleFactor": 3, "mobile": True}),
        ("Emulation.setTouchEmulationEnabled", {"enabled": True, "maxTouchPoints": 5}),
        # NOT setEmitTouchEventsForMouse: in that mode Chrome holds every synthetic mouse
        # press waiting for a gesture, so the runner's Input.dispatchMouseEvent clicks time out.
    ]):
        ws.send(json.dumps({"id": i + 1, "method": m, "params": p}))
    try:
        while ws.recv():   # keep the session (and with it the override) alive
            pass
    except Exception:
        pass

seen = set()
while True:
    try:
        for t in json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=2)):
            if t.get("type") == "page" and t["id"] not in seen and "webSocketDebuggerUrl" in t:
                seen.add(t["id"])
                threading.Thread(target=pin, args=(t["webSocketDebuggerUrl"],), daemon=True).start()
    except Exception:
        if not os.path.exists(profile):
            break
    time.sleep(0.3)
