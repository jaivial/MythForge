#!/usr/bin/env python3
"""Deterministic mobile visual audit for MythForge.

Drives a headless Chrome (via chrome-mobile.sh, the same phone profile the AI e2e
mobile pass uses) over DevTools at several real phone sizes, signs up a throwaway
company through the API, then for every screen: saves a full screenshot and checks

  overflow   no horizontal scroll (document wider than the viewport)
  occlusion  nothing fixed/sticky covers the main content column (closed drawer rail,
             bottom sheets) unless it was opened on purpose
  targets    visible interactive elements are >= 44x44 CSS px (WCAG 2.5.5 / Apple HIG);
             inline text links inside paragraphs are exempt
  inputs     text inputs render >= 16px (below that iOS Safari zooms on focus)
  clipping   no interactive element sits outside the viewport horizontally
  text       no visible text below 12px

Usage: visual-audit.py [--base URL] [--out DIR] [--devices iphone-se,pixel-7,...]
Exit status 1 when any check fails. Writes DIR/report.json, DIR/report.md and PNGs.
"""
import argparse, base64, json, os, random, shutil, string, subprocess, sys, tempfile, time, urllib.request
import websocket

DEVICES = {
    "iphone-se":      (320, 568),   # smallest supported width
    "galaxy-s8":      (360, 740),
    "iphone-15":      (390, 844),
    "pixel-7":        (412, 915),
    "iphone-15-land": (844, 390),   # landscape: short viewport, still < lg
}

PROBE = r"""(() => {
const W = innerWidth, H = innerHeight;
const vis = el => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el);
  return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none' && +s.opacity !== 0
    && r.bottom > 0 && r.top < H && r.right > 0 && r.left < W; };
const desc = el => (el.getAttribute('data-testid') ? '[' + el.getAttribute('data-testid') + '] ' : '')
  + el.tagName.toLowerCase() + ' "' + ((el.getAttribute('aria-label') || el.innerText || el.value || el.placeholder || '') + '').trim().slice(0, 40) + '"';
const out = {w: W, h: H, scrollW: document.documentElement.scrollWidth, issues: []};
if (out.scrollW > W + 1) out.issues.push({check: 'overflow', detail: 'page is ' + out.scrollW + 'px wide in a ' + W + 'px viewport'});
// occlusion: fixed/sticky boxes overlapping <main>'s content box
const main = document.querySelector('main');
if (main) {
  const m = main.getBoundingClientRect();
  for (const el of document.querySelectorAll('body *')) {
    const s = getComputedStyle(el);
    if (s.position !== 'fixed' || !vis(el) || el.closest('main')) continue;
    if (el.closest('[data-audit-open]')) continue;
    const r = el.getBoundingClientRect();
    const ox = Math.min(r.right, m.right) - Math.max(r.left, m.left);
    const oy = Math.min(r.bottom, m.bottom) - Math.max(r.top, m.top);
    if (ox > 4 && oy > 4) { out.issues.push({check: 'occlusion', detail: desc(el) + ' (fixed, ' + Math.round(r.width) + 'x' + Math.round(r.height) + ' at x' + Math.round(r.left) + ') covers ' + Math.round(ox) + 'x' + Math.round(oy) + 'px of <main>'}); }
  }
}
const sel = 'a[href],button,input:not([type=hidden]),select,textarea,[role=button],[role=tab],[role=switch],summary';
for (const el of document.querySelectorAll(sel)) {
  if (!vis(el) || el.disabled && el.tagName !== 'INPUT') { if (!vis(el)) continue; }
  const r = el.getBoundingClientRect();
  if (el.tagName === 'A' && el.closest('p')) continue;           // inline prose link
  if (el.type === 'checkbox' || el.type === 'radio') { if (el.closest('label') && el.closest('label').getBoundingClientRect().height >= 44) continue; }
  if (r.height < 43.5 || r.width < 43.5) out.issues.push({check: 'targets', detail: desc(el) + ' is ' + Math.round(r.width) + 'x' + Math.round(r.height)});
  if (r.left < -1 || r.right > W + 1) out.issues.push({check: 'clipping', detail: desc(el) + ' spans x' + Math.round(r.left) + '..' + Math.round(r.right)});
  if (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) && !/^(checkbox|radio|range|color|file)$/.test(el.type)) {
    const fs = parseFloat(getComputedStyle(el).fontSize);
    if (fs < 16) out.issues.push({check: 'inputs', detail: desc(el) + ' font-size ' + fs + 'px'});
  }
}
const small = new Set();
const tw = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
while (tw.nextNode()) { const n = tw.currentNode; if (!n.textContent.trim()) continue; const el = n.parentElement; if (!el || !vis(el) || el.closest('.sr-only')) continue;
  const fs = parseFloat(getComputedStyle(el).fontSize); if (fs < 12 && !small.has(el)) { small.add(el); out.issues.push({check: 'text', detail: desc(el) + ' ' + fs + 'px'}); } }
return out; })()"""


class Tab:
    def __init__(self, ws_url):
        self.ws = websocket.create_connection(ws_url, timeout=60, suppress_origin=True)
        self.i = 0

    def call(self, method, **params):
        self.i += 1
        self.ws.send(json.dumps({"id": self.i, "method": method, "params": params}))
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.i:
                if "error" in r:
                    raise RuntimeError(f"{method}: {r['error']}")
                return r.get("result", {})

    def js(self, expr):
        r = self.call("Runtime.evaluate", expression=expr, returnByValue=True, awaitPromise=True)
        if "exceptionDetails" in r:
            raise RuntimeError(r["exceptionDetails"].get("exception", {}).get("description", "js error"))
        return r["result"].get("value")

    def wait(self, expr, timeout=15):
        end = time.time() + timeout
        while time.time() < end:
            try:
                if self.js(expr):
                    return True
            except RuntimeError:
                pass
            time.sleep(0.25)
        return False

    def tap(self, testid):
        """A real touch tap (not el.click()) at the centre of a test id."""
        return self.tapsel(f'[data-testid="{testid}"]')

    def tapsel(self, sel):
        box = self.js(f"(()=>{{const e=document.querySelector({json.dumps(sel)}); if(!e) return null; e.scrollIntoView({{block:'center'}}); const r=e.getBoundingClientRect(); return [r.left+r.width/2, r.top+r.height/2];}})()")
        if not box:
            raise RuntimeError(f"nothing matches {sel} to tap")
        x, y = box
        self.call("Input.dispatchTouchEvent", type="touchStart", touchPoints=[{"x": x, "y": y}])
        self.call("Input.dispatchTouchEvent", type="touchEnd", touchPoints=[])
        time.sleep(0.45)   # drawer/sheet transitions are 200 ms

    def shot(self, path):
        data = self.call("Page.captureScreenshot", format="png")["data"]
        open(path, "wb").write(base64.b64decode(data))


def launch(chrome, profile):
    os.makedirs(profile, exist_ok=True)
    p = subprocess.Popen([chrome, "--headless=new", "--remote-debugging-port=0", "--remote-allow-origins=*",
                          "--window-size=1280,900", f"--user-data-dir={profile}", "--no-first-run", "about:blank"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    pf = os.path.join(profile, "DevToolsActivePort")
    for _ in range(200):
        if os.path.exists(pf) and open(pf).read().strip():
            break
        time.sleep(0.1)
    port = open(pf).read().split()[0]
    for _ in range(50):
        pages = [t for t in json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list")) if t["type"] == "page"]
        if pages:
            return p, Tab(pages[0]["webSocketDebuggerUrl"])
        time.sleep(0.1)
    raise RuntimeError("no page target")


UA = {"User-Agent": "MythForge-mobile-audit/1.0 (+e2e)", "Accept": "application/json"}


def signup(base):
    r = "".join(random.choices(string.ascii_lowercase + string.digits, k=8))
    body = json.dumps({"name": "Mobile Audit", "company_name": "Mobile Audit Co", "email": f"e2e-mobile-{r}@mythforge.test",
                       "password": "supersecret1", "template": "crm"}).encode()
    req = urllib.request.Request(base + "/api/v1/auth/signup", body, {"Content-Type": "application/json", **UA})
    d = json.load(urllib.request.urlopen(req, timeout=60))
    return d, json.dumps({"token": d["token"], "user": d["user"], "company": d["company"]})


def seed(base, token):
    for name, stage in [("Ada Lovelace", "New"), ("Grace Hopper with a very long lead name for wrapping", "Qualified")]:
        req = urllib.request.Request(base + "/api/v1/data/crm/lead", json.dumps({"name": name, "email": "lead@example.com", "stage": stage, "value": 1200}).encode(),
                                     {"Content-Type": "application/json", "Authorization": "Bearer " + token, **UA})
        urllib.request.urlopen(req, timeout=30).read()


# (name, path, setup actions, ready-expression)  setup: list of ("tap", testid) / ("js", expr)
SCREENS = [
    ("login",            "/login",                [], "document.querySelector('input[type=email]')"),
    ("workspace",        "/app",                  [], "document.querySelector('[data-testid=sidebar]')"),
    ("drawer-open",      "/app",                  [("tap", "toggle-sidebar"), ("mark", "[data-testid=sidebar]"), ("mark", "button[aria-label='Close navigation']")], "document.querySelector('[data-testid=sidebar]')"),
    # below md the assistant starts closed (as a sheet it would cover the page); open it on purpose
    ("assistant-sheet",  "/app",                  [("noexpr", "innerWidth < 768 && document.querySelector('[data-testid=assistant-panel]')"), ("closeif", "assistant-panel"), ("tap", "toggle-assistant"), ("waitfor", "document.querySelector('[data-testid=assistant-panel]')"), ("mark", "[data-testid=assistant-panel]")], "document.querySelector('[data-testid=toggle-assistant]')"),
    ("leads-table",      "/app/m/crm/lead",       [], "document.querySelector('[data-testid=records-table]')"),
    ("leads-kanban",     "/app/m/crm/lead",       [("tapsel", "[data-testid=views] button:nth-child(2)"), ("waitfor", "document.querySelector('[data-testid=kanban]')")], "document.querySelector('[data-testid=records-table]')"),
    ("record-form",      "/app/m/crm/lead",       [("tap", "new-record"), ("mark", "[data-testid=record-form]")], "document.querySelector('[data-testid=new-record]')"),
    ("contacts-empty",   "/app/m/crm/contact",    [], "document.querySelector('[data-testid=empty-state]')"),
    ("build",            "/app/build",            [], "document.querySelector('textarea')"),
    ("agents",           "/app/agents",           [], "document.querySelector('input')"),
    ("settings",         "/app/settings",         [], "document.querySelector('[data-testid=automations-card]')"),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default=os.environ.get("E2E_BASE_URL", "https://forge.myth.services"))
    ap.add_argument("--out", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".e2e-runs", "mobile-visual-" + time.strftime("%Y%m%d-%H%M%S")))
    ap.add_argument("--devices", default=",".join(DEVICES))
    ap.add_argument("--screens", default="")
    a = ap.parse_args()
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    chrome = os.path.join(os.path.dirname(os.path.abspath(__file__)), "chrome-mobile.sh")
    d, session = signup(a.base)
    seed(a.base, d["token"])
    only = set(filter(None, a.screens.split(",")))
    results = []
    for dev in a.devices.split(","):
        w, h = DEVICES[dev]
        profile = tempfile.mkdtemp(prefix="mf-mobile-audit-")
        proc, tab = launch(chrome, profile)
        try:
            tab.call("Emulation.setDeviceMetricsOverride", width=w, height=h, deviceScaleFactor=2, mobile=True)
            tab.call("Emulation.setTouchEmulationEnabled", enabled=True, maxTouchPoints=5)
            tab.call("Page.enable")
            tab.call("Page.navigate", url=a.base + "/login")
            tab.wait("document.readyState === 'complete'")
            tab.js(f"localStorage.setItem('mf_token', {json.dumps(d['token'])}); localStorage.setItem('mf_session', {json.dumps(session)}); true")
            for name, path, setup, ready in SCREENS:
                if only and name not in only:
                    continue
                if name == "login":
                    tab.js("localStorage.removeItem('mf_token'); localStorage.removeItem('mf_session'); true")
                tab.call("Page.navigate", url=a.base + path)
                ok = tab.wait(f"document.readyState === 'complete' && !!({ready})", 20)
                time.sleep(0.5)
                err = None
                try:
                    for kind, arg in setup:
                        if kind == "tap":
                            tab.tap(arg)
                        elif kind == "js":
                            tab.js(arg)
                        elif kind == "tapsel":
                            tab.tapsel(arg)
                        elif kind == "closeif":   # >= md the panel is open by default: close it first
                            if tab.js(f"!!document.querySelector('[data-testid={arg}]')"):
                                tab.tap("toggle-assistant")
                        elif kind == "noexpr":   # must NOT be present before the interaction
                            if tab.js(f"!!({arg})"):
                                raise RuntimeError(f"present before it was opened: {arg}")
                        elif kind == "waitfor":
                            if not tab.wait(f"!!({arg})", 15):   # a DOM node serialises to {} (falsy)
                                raise RuntimeError(f"never appeared after setup: {arg}")
                        elif kind == "sleep":
                            time.sleep(arg)
                        elif kind == "mark":   # opened on purpose: exempt from occlusion
                            tab.js(f"document.querySelectorAll({json.dumps(arg)}).forEach(e=>e.setAttribute('data-audit-open',''))")
                except RuntimeError as e:
                    err = str(e)
                probe = tab.js(PROBE) if ok else {"issues": [], "w": w, "h": h}
                issues = probe["issues"]
                if not ok:
                    issues.insert(0, {"check": "render", "detail": f"screen never became ready ({ready})"})
                if err:
                    issues.insert(0, {"check": "interaction", "detail": err})
                png = f"{dev}--{name}.png"
                tab.shot(os.path.join(out, png))
                results.append({"device": dev, "size": f"{w}x{h}", "screen": name, "path": path, "issues": issues, "screenshot": png})
                print(f"{'PASS' if not issues else 'FAIL'}  {dev:15} {name:16} {len(issues)} issue(s)", flush=True)
                if name == "login":
                    tab.js(f"localStorage.setItem('mf_token', {json.dumps(d['token'])}); localStorage.setItem('mf_session', {json.dumps(session)}); true")
        finally:
            try:
                os.killpg(proc.pid, 9)
            except Exception:
                pass
            shutil.rmtree(profile, ignore_errors=True)
    failed = [r for r in results if r["issues"]]
    json.dump(results, open(os.path.join(out, "report.json"), "w"), indent=2)
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write(f"# Mobile visual audit: {len(results) - len(failed)}/{len(results)} screens clean\n\nBase URL: `{a.base}`\n\n")
        f.write("| | Device | Screen | Issues |\n|---|---|---|---|\n")
        for r in results:
            det = "<br>".join(f"**{i['check']}**: {i['detail']}" for i in r["issues"][:8]) + (f"<br>…+{len(r['issues']) - 8}" if len(r["issues"]) > 8 else "")
            f.write(f"| {'✅' if not r['issues'] else '❌'} | {r['device']} {r['size']} | [{r['screen']}]({r['screenshot']}) | {det} |\n")
    print(f"\n{len(results) - len(failed)}/{len(results)} screens clean · report: {out}/report.md")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
