# Agent guidelines — Locus client

Instructions for AI coding agents working in `client/`, the Locus desktop client.
The root [`../AGENTS.md`](../AGENTS.md) covers the four rules that apply
everywhere and the repository-wide traps; this file covers only what is specific
to the client.

> **`client/` is the Locus client and it is functional.** It began as a copy of
> Clash Verge Rev v2.5.5 and now carries Locus logic in `src-tauri/src/locus/`.
> The contract docs in `client/docs/` were written before the port and several
> describe intent rather than the code — **the code wins, and the doc gets fixed
> in the same change.**

## Where to read

| If you are… | Read |
|---|---|
| setting up the dev environment, or submitting a change | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| changing anything in `client/` | [`docs/UPSTREAM-CHANGES.md`](docs/UPSTREAM-CHANGES.md) — **authoritative** on what is ours vs upstream's |
| working on the UI | [`docs/FRONTEND.md`](docs/FRONTEND.md) |
| adding or changing a translated string | [`docs/CONTRIBUTING_i18n.md`](docs/CONTRIBUTING_i18n.md) |
| touching the updater or release signing | [`docs/UPDATE-ARCHITECTURE.md`](docs/UPDATE-ARCHITECTURE.md), [`docs/SIGNING.md`](docs/SIGNING.md) |
| measuring the traffic graph's performance | [`scripts/perf/GUIDE.md`](scripts/perf/GUIDE.md) |
| cutting a release | `../AGENTS.md` → [`../docs/operate/RELEASING.md`](../docs/operate/RELEASING.md) |

## Two behaviours that are load-bearing

- **The generated proxy GROUP must not share a name with a proxy inside it** —
  mihomo rejects the whole config as a reference loop, and it is valid YAML, so
  only running the engine catches it.
- **The frozen wire names are contracts** with the deployed hub and published
  releases: `uot_port`, the `download_*`→`update_*` rename, the
  `macos_*`/`darwin-*` artifact asymmetry. Do not "tidy" them.

**Validation beyond unit tests:** `cargo test`, then run the real sidecar against
generated output (`verge-mihomo -t -f <config>`), because a config can pass every
test and still be refused by the engine.

## Traps this client has already paid for

The full stories are in [`docs/FRONTEND.md`](docs/FRONTEND.md) and
[`../docs/reference/FIXES.md`](../docs/reference/FIXES.md). The shapes to
recognise:

- **A "no read yet" state is not a negative answer.** `unknown` meant both "we
  have not read the status" and "this device is not activated", so every mount
  told activated devices to find their card. The phases are `checking` and
  `unactivated`, and only the second may tell a student to find their card.
  Before adding a state whose name means "absent", ask which *fact* is absent.
- **A throttle that re-arms on every message bounds nothing.** A coalescer whose
  window opened from each arriving frame emitted per frame; the value that
  survived was whichever frame landed when the timer was null. The rule now lives
  in `src/utils/coalescer.ts` — pure, fake-clock tested, strict trailing edge.
  Do not re-inline it.
- **A leading edge plus a trailing one is two emissions per window.** The tests
  assert the rate as a function of *time*, not of message count.
- **The lint is not the compiler.** `eslint-plugin-react-compiler` is at `error`
  and `babel-plugin-react-compiler` is **not installed**, so no component is
  auto-memoized. A green lint run says "this would compile", not "the compiler
  runs".
- **A context value with a fresh identity defeats every `memo` below it.** Check
  a `useMemo`'s dependencies for functions re-created before blaming children.
- **A diagnostic that cannot change is worse than none.** A readout that is a
  constant shown as a measurement, on the page a user consults when reporting
  jank, is worse than no readout. If you add one, prove it can move.
- **A control whose success looks like its silence reads as broken.** Prefer a
  return value that distinguishes `refreshed | busy | failed`.
- **An animation declared in a re-rendering component restarts on every render.**
  Keyframes belong in the stylesheet.
- **Progress must bracket the work it reports.** Await the thing, and register a
  waiter **before** requesting the work, or a fast completion is missed.
