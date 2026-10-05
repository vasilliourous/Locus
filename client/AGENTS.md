# Agent Guidelines — Locus client

Instructions for AI coding agents working in `client/`, the Locus desktop client.

> **`client/` is the Locus client, and it is functional.** It began as a copy of
> Clash Verge Rev v2.5.5 and now carries Locus logic in
> `src-tauri/src/locus/`: contract, activation, device fingerprint, heartbeat,
> tier→config, store, apply, runtime supervisor and the hub-mediated updater.
>
> The contract docs in `client/docs/` were written before the port and several
> now describe intent rather than the code. **Where a doc and the code disagree,
> the code wins and the doc gets fixed in the same change.**
>
> Two behaviours are load-bearing and easy to break:
>  * the generated proxy GROUP must not share a name with a proxy inside it —
>    mihomo rejects the whole config as a reference loop, and it is valid YAML,
>    so only running the engine catches it;
>  * the frozen wire names (`uot_port`, the `download_*`→`update_*` rename, the
>    `macos_*`/`darwin-*` artifact asymmetry) are contracts with the deployed hub
>    and published releases. Do not "tidy" them.
>
> Validation beyond unit tests: `cargo test`, then run the real sidecar against
> generated output (`verge-mihomo -t -f <config>`), because a config can pass
> every test and still be refused by the engine.

## Where to read

| If you are… | Read |
|---|---|
| setting up the dev environment, or submitting a change | [`CONTRIBUTING.md`](CONTRIBUTING.md) — the human-facing setup and submission guide |
| changing anything in `client/` | [`docs/UPSTREAM-CHANGES.md`](docs/UPSTREAM-CHANGES.md) — **authoritative** on what is ours vs upstream's |
| working on the UI | [`docs/FRONTEND.md`](docs/FRONTEND.md) |
| adding or changing a translated string | [`docs/CONTRIBUTING_i18n.md`](docs/CONTRIBUTING_i18n.md) |
| touching the updater or release signing | [`docs/UPDATE-ARCHITECTURE.md`](docs/UPDATE-ARCHITECTURE.md), [`docs/SIGNING.md`](docs/SIGNING.md) |
| measuring the traffic graph's performance | [`scripts/perf/GUIDE.md`](scripts/perf/GUIDE.md) — the macOS harness (`pnpm perf:build` / `perf:run` / `perf:compare`) |
| anything else in the project | `../docs/README.md` — the single documentation index |

> **Added 2026-10-05.** Four of these were reachable from nowhere: they were
> written, correct, and findable only by browsing the filesystem.

## Traps this client has already paid for

Each of these cost a real report. They are listed here because the *shape* of the
mistake recurs, and the code comments are only findable if you already know where
to look.

- **A "no read yet" state is not a negative answer.** `phase === 'unknown'` used
  to mean both "we have not read the status" and "this device is not activated",
  so every mount rendered "This device needs an activation code" for as long as one
  status read took — on activated devices. The phases are now `checking` (no read
  settled) and `unactivated` (the backend said so), and only the second may tell a
  student to find their card. Before adding any state whose name means "absent",
  ask which *fact* it is absences of.
- **A throttle that re-arms on every message bounds nothing.** The WebSocket
  coalescer opened its window from each arriving frame, so a socket faster than the
  window produced an emission per frame; and the value that survived a window was
  whichever frame landed when the timer happened to be null, not the newest one.
  The rule now lives in `src/utils/coalescer.ts` — pure, fake-clock tested, strict
  trailing edge. Do not re-inline it.
- **A leading edge plus a trailing one is two emissions per window.** Tempting for
  first-reading latency, and it silently doubles the rate for any producer whose
  interval is not a divisor of the window — 20 emissions where 11 were expected, at
  50 Hz through a 200 ms window. The coalescer's tests assert the rate as a
  function of *time*, not of message count.
- **The lint is not the compiler.** `eslint-plugin-react-compiler` is configured
  with `react-compiler/react-compiler: 'error'`, and `babel-plugin-react-compiler`
  is **not installed**, so no component is auto-memoized. A green lint run says
  "this code would compile", not "the compiler runs".
- **A context value with a fresh identity defeats every `memo` below it.** React
  contexts have no value-equality escape hatch. `refetch` was an inline closure
  per render, which invalidated `AppDataProvider`'s memoized contexts and
  re-rendered their consumers on every parent render. Check a `useMemo`'s
  dependencies for functions that are re-created before blaming the children.
- **A diagnostic that cannot change is worse than none.** The graph rendered an
  `fps` figure that was `useState(TARGET_FPS)` with both setters writing the same
  value — a constant shown as a measurement, on the page a user consults when
  reporting "it feels janky". Removed. If you add a readout, prove it can move.
- **A control whose success looks like its silence reads as broken.** The Settings
  "Refresh" was a silent no-op whenever a read was in flight (every 750 ms during
  a connect) and an invisible success otherwise. `useConnection().refresh()` now
  returns `refreshed | busy | failed` and the page states which.
- **An animation declared in a re-rendering component restarts on every render.**
  An inline `<style>` tag in `_layout.tsx` re-parsed on each traffic tick, so a
  content fade re-ran several times a second. Keyframes belong in the stylesheet,
  and an animation belongs on the element that actually appears.
- **Progress must bracket the work it reports.** `locus_check_subscription` used
  to return as soon as the heartbeat loop was *woken*, so the button's "Checking…"
  lasted one IPC round trip and reverted before the request had left the machine.
  It now awaits the beat and its outcome; `beat_now_and_wait` must register its
  waiter **before** requesting the beat, or a fast completion is missed and the
  caller sleeps until the next one.

## Cutting a release — read this before you tag anything

**CI does not bump versions.** It labels the GitHub Release and the manifest from
the git tag. So a tag whose name disagrees with the version files builds binaries
that misreport their own version, publishes them under the tag's name, and hands
every client an "update" that reinstalls the same version forever.

This shipped once (2026-09-30: `v3.2.13` tagged on a commit whose files still
said `3.2.12`). Do it in this order:

```sh
cd client
pnpm release-version X.Y.Z          # writes ALL FIVE version sites
git add -A && git commit -m "chore(release): X.Y.Z"
git tag -a vX.Y.Z -m "Locus client vX.Y.Z"
git push origin main && git push origin vX.Y.Z
```

- **The bump is one command.** `pnpm release-version` writes `package.json`,
  `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `Cargo.lock` **and
  `../docs/state.toml`**. Do not hand-edit any of them, and do not add a sixth
  site by hand — add it to `scripts/release-version.mjs` instead, so it cannot
  be forgotten.
- **Verify before pushing the tag.** `bash ../server/scripts/check-consistency.sh`
  must exit 0. Its §7 fails with `BAD client.version` when `docs/state.toml`
  disagrees with `package.json`.
- **The `verify` job guards the tag itself.** "Tag must name the version being
  built" fails a tag that disagrees with `package.json`, and `release` needs
  `verify`, so a bad tag cannot publish. It is a safety net, not the process.
- **Full procedure and the hub side:** `../docs/operate/RELEASING.md`.
