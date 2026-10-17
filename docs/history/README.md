# Historical research and plans

**These are historical.** Read them for *why*, never for *what is*. Nothing here
is maintained, and where one of them disagrees with the code, the code wins.

They exist because they hold knowledge that exists nowhere else: the threat
analysis behind the protocol choice, the market reasoning behind the pricing, and
the original plan whose dead ends are worth not repeating. Everything else about
the past — session narratives, superseded clients, the deployment that broke —
is a dated entry in [`../reference/FIXES.md`](../reference/FIXES.md), which is the
only history this project keeps current.

## What is here

| File | What it is | Superseded by |
|---|---|---|
| `Architectural-Plan.md` | The original v3 build plan: Hysteria 2, four tiers, a privileged helper. Everything in it was abandoned — the shipped design is Shadowsocks TCP + BBR + tc caps. | [`../operate/DEPLOY.md`](../operate/DEPLOY.md), [`../client/docs/ARCHITECTURE.md`](../../client/docs/ARCHITECTURE.md) |
| `Business-Plan.md` | The commercial model as first written: middlemen, cash, tiers, the comparison against the incumbent. | [`../business/`](../business/) — this is the strategy, the plan is what is live |
| `attacker-perspective.md` | An adversarial/insider-threat analysis of covert telemetry and tracking in VPN products. | Nothing — the *reason* no identity is stored; see [`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md) |
| `defender-perspective.md` | Defense-in-depth research answering the above. | Nothing — same |
| `comprehensive-vpn-blocklist.md` | N4L / Palo Alto fingerprint research: how schools detect VPNs. | Nothing — the rationale for the transport choice |
| `school-vpn-blocking-implementation.md` | How N4L implements blocking at the network level. | Nothing — same |
| `LAYOUT.md` | The before/after record of every documentation reorganisation. | [`../README.md`](../README.md) for the *current* layout |

## What was removed, and how to get it back

The 2026-09 session records (five files, ~950 lines: the index, the what-we-did
narrative, the gotchas, the update arc and the handoff) were deleted on
2026-10-05. They described one conversation, not the project, and the durable
rules they produced live where they are enforced — the pitfalls are in
[`../CONTEXT.md`](../CONTEXT.md) §6, the method in
[`../reference/DEBUGGING-METHOD.md`](../reference/DEBUGGING-METHOD.md), and the
defects in [`../reference/FIXES.md`](../reference/FIXES.md).

Older material from the 2026-08 cleanup is also not in-repo. Each is one command
away:

```sh
git log --oneline --all -- docs/history/SESSION-GOTCHAS.md   # find the last commit that had it
git show <commit>^:docs/history/SESSION-GOTCHAS.md           # read it
```

- **Removed 2026-10-05:** `SESSION-INDEX.md`, `SESSION-WHAT-WE-DID.md`,
  `SESSION-UPDATE-ARC.md`, `SESSION-GOTCHAS.md`, `SESSION-HANDOFF-2026-09-23.md`.
- **Removed 2026-08 and never archived:** `originals/Actionable-Plan.md`, the
  `v3/` and `simplified/` alternative doc sets, `modular-vps/`.

Files keep their original names so old references and `git log` stay coherent.
**Broken links inside these files are expected** — several point at documents
that lived in a private research directory and were never in this repo. Treat
them as "see private research", not as repo breakage.
