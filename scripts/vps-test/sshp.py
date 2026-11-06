#!/usr/bin/env python3
"""Password SSH runner for the test VPS (no sshpass on this host).

Usage:
    python3 scripts/vps-test/sshp.py [--host H] [--user U] [--pw P] [--timeout S] "remote command"
    python3 scripts/vps-test/sshp.py --file local.sh [--put remote.sh] [--chmod]
    python3 scripts/vps-test/sshp.py --get remote-path > local-file

Reads the default host/user/password from the environment so the password is not
written into a command line (which would land in shell history / process list):

    VPS_HOST=<hub-host> VPS_USER=root VPS_PW='...' python3 scripts/vps-test/sshp.py 'uptime'

VPS_HOST has NO default. It used to be a hardcoded literal here, which meant an
operator (or an agent) running this script without VPS_HOST silently targeted a
stale address instead of the current hub. The host is a deployment fact, not a
constant in a script: take it from the environment, and fail loudly when absent.
Never write an IP address into this file (AGENTS.md rule 2).

Never prints the password. Streams the remote session's stdout/stderr verbatim.
Password auth is forced (PreferredAuthentications=password) so a stale agent key
cannot silently pick a different identity.
"""

import argparse
import os
import pty
import select
import subprocess
import sys
import time

# ── Where credentials come from ──
# Priority: an explicit environment variable, then the decrypted secrets bundle.
# The bundle is the source of truth (DOMAIN, VPS_HOST, VPS_USER, VPS_PW) so that
# neither a credential nor a host address lives in this file. Reading it costs one
# `age -d` per invocation and requires LOCUS_SECRETS_PASSPHRASE — the same
# contract as server/setup.sh; see docs/operate/SECRETS-MANAGEMENT.md.
_HERE = os.path.dirname(os.path.abspath(__file__))
SECRETS_AGE = os.path.abspath(os.path.join(_HERE, "..", "..", "server", "secrets.env.age"))
SECRETS_DECRYPT = os.path.join(os.path.dirname(SECRETS_AGE), "scripts", "secrets-decrypt.sh")

_bundle_cache = {}


def _from_bundle(name):
    """Look `name` up in the decrypted secrets bundle. Never printed, never logged.

    Requires LOCUS_SECRETS_PASSPHRASE (or _FILE) to be set. If it is not, return
    "" immediately rather than invoking the decryptor: `age -d` opens /dev/tty and
    would block forever on an unattended run. Same contract as server/setup.sh.
    """
    if not _bundle_cache:
        if not os.path.exists(SECRETS_DECRYPT):
            return ""
        if not (os.environ.get("LOCUS_SECRETS_PASSPHRASE") or
                os.environ.get("LOCUS_SECRETS_PASSPHRASE_FILE")):
            _bundle_cache["__err__"] = (
                "no passphrase: set LOCUS_SECRETS_PASSPHRASE to read secrets.env.age"
            )
            return ""
        try:
            # stdin must be closed and stdout captured: age wants a pty, and the
            # decryptor allocates one itself. A hard timeout means a wedged age
            # surfaces as an error here, never as a hang in the caller.
            proc = subprocess.run(
                ["bash", SECRETS_DECRYPT],
                stdin=subprocess.DEVNULL,
                capture_output=True, text=True, timeout=60,
            )
        except subprocess.TimeoutExpired:
            _bundle_cache["__err__"] = "decrypting secrets.env.age timed out"
            return ""
        except (OSError, subprocess.SubprocessError) as exc:
            _bundle_cache["__err__"] = f"could not decrypt secrets.env.age: {exc}"
            return ""
        if proc.returncode != 0:
            # A wrong passphrase must not leak into our own stdout — stay silent
            # but remember that we failed, so the caller can say so.
            _bundle_cache["__err__"] = "secrets.env.age did not decrypt (wrong passphrase?)"
            return ""
        for line in proc.stdout.splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, val = line.partition("=")
            _bundle_cache[key.strip()] = val.strip().strip("\"'")
    return _bundle_cache.get(name, "")


def _resolve(*names):
    """First non-empty of: env var, then the secrets bundle — in the order given."""
    for name in names:
        val = os.environ.get(name, "")
        if val:
            return val
    for name in names:
        val = _from_bundle(name)
        if val:
            return val
    return ""


SSH_OPTS = [
    "-tt",
    "-o", "StrictHostKeyChecking=accept-new",
    "-o", "ConnectTimeout=15",
    "-o", "ServerAliveInterval=15",
    "-o", "ServerAliveCountMax=6",
    "-o", "PreferredAuthentications=password",
    "-o", "PubkeyAuthentication=no",
    "-o", "NumberOfPasswordPrompts=1",
    "-o", "LogLevel=ERROR",
]


def run(host, user, pw, argv, timeout, stdin_path=None):
    if stdin_path:
        opts = [o for o in SSH_OPTS if o not in ("-tt",)]
        with open(stdin_path, "rb") as fh:
            pid, fd = pty.fork()
            if pid == 0:
                os.dup2(fh.fileno(), 0)
                os.execvp("ssh", ["ssh"] + opts + [f"{user}@{host}"] + argv)
    else:
        pid, fd = pty.fork()
        if pid == 0:
            os.execvp("ssh", ["ssh"] + SSH_OPTS + [f"{user}@{host}"] + argv)

    out = bytearray()
    sent = False
    start = time.time()
    while True:
        if time.time() - start > timeout:
            try:
                os.kill(pid, 9)
            except ProcessLookupError:
                pass
            out += b"\n[sshp] TIMEOUT after %ds\n" % timeout
            break
        r, _, _ = select.select([fd], [], [], 1.0)
        if r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            out += data
            if not sent and (b"assword:" in data or b"Password:" in data):
                os.write(fd, (pw + "\n").encode())
                sent = True
                # Gag the terminal echo of the password itself.
                time.sleep(0.4)
        else:
            try:
                wpid, _ = os.waitpid(pid, os.WNOHANG)
                if wpid:
                    break
            except ChildProcessError:
                break
    return bytes(out)


def main():
    ap = argparse.ArgumentParser()
    # Defaults are resolved AFTER parsing (env, then the secrets bundle) so that
    # --host/--user/--pw still win, and so a bundle read is only done when needed.
    ap.add_argument("--host", default="")
    ap.add_argument("--user", default="")
    ap.add_argument("--pw", default="")
    ap.add_argument("--timeout", type=int, default=180)
    ap.add_argument("--file", help="local script to pipe to `bash -s` on the remote")
    ap.add_argument("--put", help="remote path to upload --file to (implies --get-put mode)")
    ap.add_argument("--chmod", action="store_true", help="chmod +x the uploaded file")
    ap.add_argument("--get", help="remote path to download to stdout")
    ap.add_argument("cmd", nargs="*", help="remote command words")
    args = ap.parse_args()

    args.host = args.host or _resolve("VPS_HOST", "DOMAIN")
    args.user = args.user or _resolve("VPS_USER") or "root"
    args.pw = args.pw or _resolve("VPS_PW")

    if not args.host:
        sys.stderr.write(
            "sshp: no host (set VPS_HOST or pass --host).\n"
            "  There is deliberately no default: a stale hardcoded host sent this\n"
            "  script at the wrong box. See docs/operate/OPS.md.\n"
        )
        return 2

    if not args.pw:
        err = _bundle_cache.get("__err__", "")
        sys.stderr.write("sshp: no password (set VPS_PW, or put VPS_PW in the bundle)\n")
        if err:
            sys.stderr.write(f"  bundle: {err}\n")
        return 2

    if args.get:
        argv = ["cat", args.get]
        out = run(args.host, args.user, args.pw, argv, args.timeout)
        sys.stdout.buffer.write(out)
        return 0

    if args.file and args.put:
        # Upload: cat localfile | ssh 'cat > remote'
        argv = ["cat", ">", args.put]
        if args.chmod:
            argv = ["sh", "-c", f"cat > {args.put} && chmod +x {args.put}"]
        out = run(args.host, args.user, args.pw, argv, args.timeout, stdin_path=args.file)
        sys.stdout.write(out.decode(errors="replace"))
        return 0

    if args.file:
        # Run a local script remotely without leaving it on disk.
        argv = ["bash", "-s"]
        out = run(args.host, args.user, args.pw, argv, args.timeout, stdin_path=args.file)
        sys.stdout.write(out.decode(errors="replace"))
        return 0

    if not args.cmd:
        sys.stderr.write("sshp: nothing to do\n")
        return 2
    out = run(args.host, args.user, args.pw, args.cmd, args.timeout)
    sys.stdout.write(out.decode(errors="replace"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
