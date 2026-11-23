# Documentation audit — what the doc set gets wrong, and what a guard cannot see

```
audience:    all
status:      live
authoritative-for: the known blind spots of the doc set, and the one guard change still open
verified-against: this checkout at 2026-11-23; check-consistency.sh after each change
```

A dated observation, not a rule. The live rules are [`../README.md`](../README.md)
rules 1–5 and the §-numbered guards; this page records where they currently bite
and where they cannot. Per the ownership rule it cites, the *live* list of what is
unfinished is [`STILL-OPEN.md`](STILL-OPEN.md) — not repeated here.

## The standing defect class

**The guards pass, and the set is still wrong in places they cannot see.** A guard
only sees what someone encoded into it; the recurring defect is a *correct
sentence that stays in place while the code moves underneath it*.

Three incidents of that class, each of which produced a rule now enforced
somewhere:

- **A guard enforced the defect** (§23(g)/§27(g) once required `README.md` to
  restate the tier caps, so the only way to pass was to keep a second hand-maintained
  copy). Now asserts the negative: no cap string in `README.md` at all.
- **`.md`-only scanning** let two code comments point at the client architecture
  doc for a year after it moved to `archive/`. §8 now scans the source trees with
  the same regex.
- **A count in prose** ("three version sites", while the bump set was five) survived
  three overhaul passes. Both readings were true of different sets, so the fix was
  not a find-and-replace: stop writing the number and link `[client.version_sites]`,
  which §17 recomputes.

## What is still open

**§10 cannot see word-numbers.** It passed on "three version sites" because
*three* is a word, not a digit — a guard that cannot see the defect it exists for
reads exactly like a guard that passed. Extending it to spelled-out numbers where
the sentence also names the version sites is the highest-leverage guard change left.

*That is no longer the highest-leverage gap.* As of 2026-11-22 the larger one was
**scope, not vocabulary**: root `*.md` sat outside §8's, §10's and §28's walks, so
the front door was orphaned and unguarded while §28 printed a completeness claim
that did not cover it. §28/§8/§10 now walk root `*.md` and the word-number gap
above is unchanged and still open.

**Scope bit a second time on 2026-11-23, one level in.** Nested
`client/src-tauri/**/*.md` was in no walk at all, so the icon migration's new
authoritative document (`icons/README.md`) was orphanable with nothing failing.
§28 now walks it. The lesson generalises, and is written into the guard: **an
exemption list is safe, a scope that stops at a fixed depth is not** — the next
document lands one level deeper than whoever wrote the walk.

**The third incident, and the one no walk would have caught.** `COLOURS.md` §8
described the macOS template ink as white while `generate-icons.mjs` draws it
black — a correct-looking sentence whose subject was inverted. §49 passed because
it asserts where a colour literal *lives*, not whether a sentence about that
literal is *true*; `generate-icons.mjs --check` passed because it compares bytes
and cannot read prose. Closing that class word-by-word is not tractable, but where
a value has exactly one home in code and one home in prose, **the pair can be
compared** — and §54 now does so for the template ink. The shape to look for
next: an `x: '<value>'` in a generator and a paragraph explaining `'<value>'` in a
document, with nothing tying the two together.

## Shape of the set, and why it is like that

| Set | Contract | Read it as |
|---|---|---|
| `operate/` | Procedure an operator follows | True now |
| `reference/` | Contracts, plus two records | Live parts true now; `FIXES.md` dated, never current |
| `business/` | Commercial/policy plan | Decided, not necessarily deployed |
| `business/redesign/` | `design-record` | Why, never as a spec for what is |
| `archive/`, `history/` | Retired | Why, never for what is |

`FIXES.md` and `STILL-OPEN.md` are large on purpose — an incident log's value is
searchability, and the "what is not verified" list is what makes the rest honest.
Their only failure mode is contradicting a live document, which is checked
separately.

## Verified / not verified

- **Verified by running:** `bash server/scripts/check-consistency.sh` after each
  change in this pass, including the guard edits.
- **Verified by reading:** the §23(g)/§27(g) guard inversion, the §8 `.md`-only
  scan, and the version-count contradiction.
- **Not verified:** that any `operate/` document is currently true of the live
  system — the hub was down through this pass, so this was impossible rather than
  skipped. Treat every procedure there as "true as written, unconfirmed against a
  running box".

*Revised 2026-11-22. When this page and the code disagree, the code wins and this
page is the bug.*
