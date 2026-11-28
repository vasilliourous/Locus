# The mark harness

```
audience:    builder
status:      live
authoritative-for: how to LOOK at the sidebar marks and the activation brand block
```

`verify.mjs` is the guard: it mounts the real `.the-logo` block in a real browser
and asserts the things that have actually gone wrong.

```sh
pnpm exec vite --config harness.vite.config.mts --port 3100 &   # from client/
node .harness/verify.mjs          # the marks
node .harness/verify-states.mjs   # the stopped page, and "No expiry" vs a countdown
```

`verify-states.mjs` mounts two SHIPPED screens: `SuspendedScreen` (the screen the
real `gateScreen` selects for a suspended device) and `LayoutSubscription` (with
only its data source replaced — `subscription-fixture.ts`, aliased in
`harness.vite.config.mts` — because the real hook needs the Tauri IPC). It is the
client-app half of requirements R3 (a release puts the student on the stopped
page, showing the hub's own sentence) and R5 (a hub-confirmed absence of a date
reads "No expiry" and does not count down). Screenshots land in `shots/`.

**Why a browser and not `vitest`.** Every defect in this family is a *layout*
failure, and none of them is visible in the DOM:

| Symptom | Cause |
|---|---|
| Wordmark 0×0, invisible | an SVG with no `width`/`height` and no `aspect-ratio` resolves `auto` to 0, not from its viewBox |
| Glyph at 24 px | MUI's `.MuiSvgIcon-root { width: 1em; height: 1em; font-size: 1.5rem }` — a rule sizing only width/height is left at the em default |
| Letters squashed | a definite `height` plus `max-width: 100%` coerces the box to 5.25:1 where the artwork is 8:1 |
| Glyph dwarfs the name | 36 px box against a 16.4 px wordmark is an ink ratio of 2.0:1 |

`vitest` would pass all four. The first two shipped, and the third and fourth were
introduced and only caught here.

**The numbers are derived, not chosen.** `.layout-content__left` is
`flex: 1 0 200px` against a `flex: 1 1 100%` sibling, so it is 200 px at *every*
window size — measured at 520 (the app's own minimum), 940 and 1400. The content
slot is 159 px, and the wordmark gets what the glyph and the 8 px gap leave. If you
change one mark, re-run this and the other moves with it.

`pnpm exec playwright install chromium` is a one-time step; `playwright` is a
devDependency.
