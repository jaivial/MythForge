# Mobile visual tests

Two complementary passes, both against the deployed app (`https://forge.myth.services`).

## 1. AI scenarios on a phone (`mobile/tests/*.e2e.yaml`)

The regular e2e runner (`mini-agent-rs e2e`, worker + judge `zai/glm-5.3-flash`) driving
a Chrome that behaves like a phone:

```bash
MINITUI_CHROME=$PWD/mobile/chrome-mobile.sh \
  mini-agent-rs e2e run -C mobile/e2e.yaml -j 3 -l 6 --step-limit 60
```

`chrome-mobile.sh` starts the real Chrome with touch events, a coarse pointer, no hover and
an iPhone user agent, plus `phone-sidecar.py`, which pins every page to a 390x844 mobile
viewport over DevTools (headless Chrome will not open a window narrower than 500 px, so
`--window-size` alone is not enough). Every scenario first asserts a layout fact that only
holds on a phone (the drawer is closed off-canvas, the assistant sheet starts closed), so
a run that silently fell back to the desktop layout fails instead of passing.

## 2. Deterministic pixel audit (`mobile/visual-audit.py`)

No model involved. It signs up a throwaway company, seeds two leads, and screenshots
11 screens on 5 devices (iPhone SE 320x568, Galaxy S8 360x740, iPhone 15 390x844,
Pixel 7 412x915, iPhone 15 landscape 844x390), checking on each one:

| check | rule |
|---|---|
| overflow | no horizontal page scroll |
| occlusion | no fixed layer covers `<main>` unless it was opened on purpose |
| targets | visible controls are at least 44x44 CSS px (inline prose links exempt) |
| inputs | text inputs are at least 16px (otherwise iOS Safari zooms on focus) |
| clipping | no control sits partly outside the viewport |
| text | no visible text below 12px |
| interaction | real touch taps open the drawer, assistant sheet, form and kanban |

```bash
python3 mobile/visual-audit.py                       # all devices and screens
python3 mobile/visual-audit.py --devices iphone-se --screens record-form,leads-kanban
```

It writes `report.md`, `report.json` and one PNG per device and screen under
`.e2e-runs/mobile-visual-<timestamp>/`, and exits 1 if any check fails.
