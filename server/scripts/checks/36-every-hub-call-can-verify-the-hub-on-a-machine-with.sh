#!/usr/bin/env bash
# Section 36 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_36() {
    echo "36. Every hub call can verify the hub on a machine with an old root store"

    # ── WHY THIS GUARD EXISTS ──
    #
    # On 2026-10-18 a student on Windows could not activate, on either plan, every
    # time: "client error (Connect): invalid peer certificate: Expired" for a hub
    # certificate issued that morning and valid for three months.
    #
    # The cause was not the hub. `chrysalis/activation.rs` built its own
    # `reqwest::Client`, and a bare builder uses `rustls-platform-verifier`, which on
    # Windows reads ONLY the Windows certificate store. The hub's chain is anchored at
    # `ISRG Root X2`, an ECDSA root that reaches Windows machines solely through
    # root-certificate update servicing. A machine that has not taken those updates
    # has no anchor; rustls does not chase the certificate's AIA extension; and
    # webpki's "no path to an anchor" surfaces as `InvalidCertificate(Expired)`.
    #
    # The lesson is the one this repository keeps paying for: a policy copied into
    # several places drifts. Activation, heartbeat and the update download each had
    # their own client, and only the shared one in `utils::network` had a fallback.
    #
    # The rule this asserts: a hub call is made through `chrysalis::hub::client`, which
    # tries the platform store and falls back to the compiled-in roots. A hand-built
    # `reqwest::Client::builder()` in the hub-facing modules is the defect.
    #
    # VERIFIED TO FAIL: adding `.build()` on a `reqwest::Client::builder()` to
    # `chrysalis/heartbeat.rs` produces the BAD line below. Removing the bundled-roots
    # fallback from the hub client produces the second.
    #
    # ── THE CLIENT MOVED, AND THIS GUARD HAD TO LEARN WHERE ──
    #
    # `chrysalis/hub.rs` became `chrysalis/hub/{mod,client,roots}.rs` so that the "which
    # store first" DECISION could be tested as a pure function of the platform (see
    # the doc comment in `roots.rs`: a `cfg!`-based test only ever asserts the host's
    # branch, so it passes on Linux CI whatever the macOS rule says). The rule this
    # section enforces is unchanged, but three things it keyed on the FILE NAME broke
    # at once, and all three read as genuine drift:
    #
    #   * the exemption was `/hub\.rs$`, so the hub module's OWN files were reported
    #     as rogue clients;
    #   * `HUB_RS` was one path, so the "fallback removed" and "crypto provider"
    #     assertions failed against a file that legitimately no longer exists;
    #   * the fallback's evidence moved with the code — `webpki_roots` is named in
    #     `hub/client.rs` now, and the `builder_with_provider` pin is in the same
    #     file.
    #
    # So the guard keys on the DIRECTORY `chrysalis/hub/`, accepts the single-file form
    # (`hub.rs`) for any checkout that predates the split, and requires that ONE of
    # them exists — "the hub client is gone" must still fail, which is the assertion
    # that made this section worth having.
    # ─────────────────────────────────────────────────────────────
    LOCUS_DIR="$REPO/client/src-tauri/src/chrysalis"
    HUB_DIR="$LOCUS_DIR/hub"

    if [ ! -d "$LOCUS_DIR" ]; then
        warn "no client/src-tauri/src/chrysalis — this guard cannot run"
    else
        # Only the hub-facing modules: these are the ones that talk to the hub over
        # TLS. The hub client itself — `chrysalis/hub/` as a directory, or `hub.rs` as the
        # pre-split single file — is where the client belongs and is allowed to build
        # one.
        #
        # ONE EXEMPTION, and it is narrow: `activation_tunnel.rs` talks **plain HTTP
        # over a loopback SOCKS proxy**, inside the Shadowsocks stream. There is no TLS
        # handshake on that path at all, so no root store is consulted and the failure
        # this guard exists for — `InvalidCertificate(Expired)` from an unpatched
        # Windows store — cannot occur. Routing it through `hub::client` would in fact
        # be wrong: that client sets `no_proxy()` precisely so a hub call never goes
        # through a tunnel, which is the opposite of what this module needs.
        #
        # The exemption is by FILE and is asserted to be the tunnel module, not
        # granted by pattern, so a second hand-built client cannot hide behind it. The
        # module's own tests pin that its base URL is `http://` and its proxy is
        # `socks5h://`, which is what makes the exemption true rather than assumed.
        TUNNEL_CLIENT="$LOCUS_DIR/activation_tunnel.rs"
        rogue_clients=$(grep -rlE 'reqwest::Client::builder\(\)' "$LOCUS_DIR" 2>/dev/null \
            | grep -vE '/hub\.rs$|/hub/' \
            | grep -vF -x "$TUNNEL_CLIENT" || true)
        if [ -n "$rogue_clients" ]; then
            for f in $rogue_clients; do
                bad "hub-facing module builds its own reqwest client: ${f#"$REPO"/}"
                bad "  A bare client trusts only the OS certificate store. On a Windows"
                bad "  machine without the ISRG Root X2 update that cannot verify the hub,"
                bad "  and the student sees \"invalid peer certificate: Expired\" for a"
                bad "  valid certificate. Use chrysalis::hub::client instead."
            done
        else
            ok "no hub-facing module builds its own reqwest client"
        fi

        # The hub client, in either shape. A directory should be preferred, but a
        # checkout from before the split is still a valid tree to check.
        HUB_SRC=$(find "$LOCUS_DIR" -maxdepth 2 \( -path "$HUB_DIR/*.rs" -o -path "$LOCUS_DIR/hub.rs" \) -type f 2>/dev/null | sort)
        if [ -z "$HUB_SRC" ]; then
            bad "neither chrysalis/hub/ nor chrysalis/hub.rs exists — nothing provides the root-store fallback"
        else
            # The assertion is about the client that BUILDS the TLS config, and that is
            # `client.rs` once the module is split. Searching the whole hub module
            # would also match `roots.rs`'s doc comments that quote the code shape, so
            # the file that builds a client is identified by its content, not by a
            # hard-coded name: any hub file that constructs a `ClientConfig`.
            cfg_src=$(printf '%s\n' "$HUB_SRC" | while IFS= read -r f; do
                grep -qE 'ClientConfig::builder' "$f" && echo "$f"
            done)
            [ -n "$cfg_src" ] || cfg_src="$HUB_SRC"

            # ── WHY THESE TWO CHECKS LOOK CONVOLUTED ──
            #
            # The obvious form is a pipeline:
            #
            #     printf '%s\n' "$HUB_SRC" | while read f; do grep -q X "$f" && echo f; done | grep -q .
            #
            # It is WRONG, and it fails in the way this repository keeps paying for:
            # `grep -q` exits as soon as it matches, the writer (`echo`) gets SIGPIPE,
            # and `pipefail` makes the whole pipeline report failure — so a check that
            # FOUND what it wanted reports "not found". Measured: the bundled-roots
            # assertion reported BAD against a tree whose `client.rs` plainly contains
            # `webpki_roots::TLS_SERVER_ROOTS`.
            #
            # So the loop writes to a variable and the test looks at the variable.
            # No early exit, no SIGPIPE, and the exit status is `[ -n ... ]`'s.
            has_bundled=""
            cfg_without_provider=""
            while IFS= read -r f; do
                [ -n "$f" ] || continue
                grep -q 'webpki_roots::TLS_SERVER_ROOTS' "$f" && has_bundled="$f"
            done <<EOF
$HUB_SRC
EOF
            while IFS= read -r f; do
                [ -n "$f" ] || continue
                # The panic that proves a fallback was never run: `ClientConfig::builder()`
                # refuses to guess a crypto provider, and this crate enables only `ring`.
                # `builder_with_provider` names it and is the only accepted form.
                grep -qE 'rustls::ClientConfig::builder\(\)' "$f" && cfg_without_provider="$f"
            done <<EOF
$cfg_src
EOF

            if [ -z "$has_bundled" ]; then
                bad "the hub client no longer uses the bundled roots — the Windows fallback is inert"
            else
                ok "the hub client has a bundled-roots fallback (webpki_roots)"
            fi

            if [ -n "$cfg_without_provider" ]; then
                bad "the hub client calls rustls::ClientConfig::builder(), which panics without a"
                bad "  process-level crypto provider — the fallback would crash the activation"
                bad "  screen instead of fixing it. Use builder_with_provider."
            else
                ok "the bundled-roots TLS config names its crypto provider"
            fi
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
