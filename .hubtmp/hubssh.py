#!/usr/bin/env python3
"""Run a command on the hub over password SSH.

Exists because this machine has ssh/scp but no sshpass and no rsync, and the hub
is password-authenticated. pexpect drives the prompt so the password never
appears in argv, in the process table, or in shell history.

Usage: hubssh.py <host> <command...>
Exit:  the remote exit status.
"""
import sys

import pexpect

SECRETS = "/tmp/.chrysalis-secrets.env"


def read_secrets():
    vals = {}
    with open(SECRETS, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, _, v = line.partition("=")
            vals[k] = v
    return vals


def main():
    if len(sys.argv) < 3:
        sys.exit("usage: hubssh.py <host> <command...>")
    host = sys.argv[1]
    remote_cmd = " ".join(sys.argv[2:])
    s = read_secrets()
    user, pw = s["VPS_USER"], s["VPS_PW"]

    argv = [
        "ssh",
        "-o", "StrictHostKeyChecking=accept-new",
        "-o", "UserKnownHostsFile=/dev/null",
        "-o", "ConnectTimeout=20",
        f"{user}@{host}",
        remote_cmd,
    ]
    child = pexpect.spawn(argv[0], argv[1:], timeout=900, encoding="utf-8")
    i = child.expect([r"(?i)password:", r"(?i)yes/no", pexpect.EOF, pexpect.TIMEOUT])
    if i == 1:
        child.sendline("yes")
        child.expect([r"(?i)password:", pexpect.EOF], timeout=30)
        child.sendline(pw)
    elif i == 0:
        child.sendline(pw)
    child.logfile_read = sys.stdout
    child.expect(pexpect.EOF, timeout=900)
    child.close()
    sys.exit(child.exitstatus if child.exitstatus is not None else 1)


if __name__ == "__main__":
    main()
