# Client hardening — the retired Wails client vs v4

> **STATUS: describes the RETIRED Wails client. Not the shipping product.**
> Extracted from `docs/CONTEXT.md` §3 during the 2026-09 documentation overhaul.
> Neither `legacy/wails-client/` nor `legacy/v4/` ships. The live client is
> `client/` (Tauri 2 on Clash Verge Rev); read
> [`../../client/docs/ARCHITECTURE.md`](../../client/docs/ARCHITECTURE.md) and
> `../../client/docs/LOGIC-INVENTORY.md` for that, and
> [`ARCHITECTURE-wails.md`](ARCHITECTURE-wails.md) for the archived design.
>
> This material is kept only because it records the behavioural contract the
> Tauri client must reproduce — the same reason `legacy/wails-client/ARCHIVED.md`
> exists. **Nothing below applies to the shipping client**, which inherits
> upstream Verge's hardening instead.

---

The `legacy/wails-client/` codebase is a hardened evolution of the v4 source:

- **Context propagation** for all network operations
- **Panic recovery** with stack traces at global and goroutine level
- **Signal handling** (SIGINT/SIGTERM → graceful shutdown → force kill after 5s)
- **Input validation** on all user-facing entry points
- **Atomic file writes** with `.tmp` + `rename()` strategy and 3-deep backup rotation
- **Process health monitoring** with auto-restart (max 3 restarts in 5 minutes)
- **Download validation** with min/max size + SHA256 checksum
- **Thread safety** via `sync.Mutex`/`sync.RWMutex` on all shared state
- **Heartbeat jitter** (±10%) to prevent thundering herd
- **Error wrapping** throughout with `fmt.Errorf("...: %w", err)`
- **Callback timeout guard** (5s max for heartbeat callbacks)
- **Tunnel watchdog** — independent 10s probe loop that verifies traffic actually
  flows, with an escalation ladder and a bounded retry count
- **Downgrade-proof updater** — numeric version comparison (`1.10.0 > 1.9.0`;
  build metadata ignored), so only strictly-newer builds are ever applied
- **Hub TLS pinning** — SPKI allow-list on every hub connection (`internal/pinned`)
- **Stale-host self-heal** — on start and on connect, orphaned `sing-box`
  processes are killed and a leftover `locus0` TUN is removed, rather than
  telling a student to go use Task Manager
- **Code normalisation** — stored/seeded codes are rewritten to the canonical
  `RQ-XXXX-XXXX-XXXX-C` form at startup, so legacy installs keep heartbeating
