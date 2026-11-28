# Console browser verification

These scripts drive the **real** console in headless Chromium and report what an
operator would hit. They exist because "it builds" and "it typechecks" say nothing
about whether a button is clickable, a column is readable, or a screen tells the
truth — and every defect listed at the bottom of this file was found by running
them, not by reading the code.

They are **not** part of CI. They need a browser and a running hub, so they are a
tool you run when you change the console.

## What you need

```sh
# 1. Playwright (once). The package and its browser are not repo dependencies —
#    this is an operator tool, not a build input.
mkdir -p ~/.local/share/console-verify && cd ~/.local/share/console-verify
npm init -y && npm i playwright && npx playwright install chromium

# 2. Let these scripts find it:
export NODE_PATH=~/.local/share/console-verify/node_modules
ln -s ~/.local/share/console-verify/node_modules server/console/verify/node_modules

# 3. PocketBase is NOT required — `mock-hub.mjs` stands in for the hook.
```

## Running

```sh
cd server/console/verify

node mock-hub.mjs &                       # the stand-in hub on :8777
../node_modules/.bin/vite --config vite.verify.config.ts &   # the console on :5199

node drive.mjs        # 48 functional checks: routes, filters, keyboard, deep links
node workflows.mjs    # 16 mutating flows: renew, save, guard, bulk, create, back
node audit.mjs        # contrast (WCAG AA), touch targets, focus visibility
node visual.mjs       # layout facts: overflow, clipping, rhythm, type sizes
node requirements.mjs # R2–R6 + D1: probe result and last-run line, direct
                      # overrides, no no-op fields, Term/Expiry at top level,
                      # History/Record demoted
node invariants.mjs   # I1–I5: reachability, state round-trips, no clipped
                      # strings, no inert controls, label truth
```

`requirements.mjs` MUTATES the mock's fixture (a release, an in-term override, an
expiry set and cleared), so restart `mock-hub.mjs` before each run of it.

Screenshots land in `shots/`.

## What each suite answers

| Script | The question it answers |
|---|---|
| `drive.mjs` | Does every destination, filter, deep link and keyboard shortcut actually work? |
| `workflows.mjs` | Does a renewal/save/release change what the screen shows, and does the unsaved-changes guard hold? |
| `audit.mjs` | Is text readable (contrast) and is every control big enough to hit? |
| `visual.mjs` | Does anything overflow, clip silently, or use unreadably small type? |
| `requirements.mjs` | Does the probe run and report, does an override change the state the panel/list show, and are the deleted no-op controls really gone? |
| `invariants.mjs` | Is every property two actions away, does every state action move the badge, is anything clipped, and is every control's label true? |

The full 36-cell state matrix is not a browser test: it is
`server/scripts/state-matrix.js`, run by `check-consistency.sh` §67, because it
drives the real hook handlers rather than the stand-in hub.

`mock-hub.mjs` implements the `admin/console` actions from
`server/pb_hooks/admin_console.pb.js` with the same field names and refusal
cases, so the SPA exercises its real code paths. **It does not prove the hook** —
that is `check-consistency.sh` and the live hub.

The fixture is deterministic (fixed codes, clock-relative offsets), so the same
screenshot is valid tomorrow.

## Defects these found

Kept here because each one is a class, not an incident.

1. **The Renewals board under-reported the book.** It fetched a 365-day window,
   which `codes.list` deliberately filters to exclude lapsed, suspended and
   never-expiring codes — then rendered lanes for exactly those. Result: 8 of 15
   codes, with the "Lapsed" and "Stopped" lanes permanently empty and a footnote
   claiming otherwise. `drive.mjs` asserts the lane counts sum to the hub total.
2. **The drawer's submit button was unclickable.** A toast stack, and later the
   inspector, sat above the drawer at the same stacking level; the click
   dismissed a toast or hit the inspector's history list instead of committing.
3. **The inspector went stale after a renewal.** It re-fetches only when the
   selected code changes, so renewing from the drawer left the panel showing the
   old expiry while the table showed the new one.
4. **The table scrolled sideways at 1440px.** Percentages, `max-width: 1px`
   cells and a duplicate `max-width: 1240px` on `.main` interacted; the code
   column clipped mid-identifier.
5. **A duplicate `.main` rule** silently overrode the canonical one, capping the
   console at 1240px.
6. **Tables overflowed the page on a phone** because only the Codes table had a
   scroll container.
