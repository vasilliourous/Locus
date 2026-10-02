// /api/device-recognise — RETIRED. Kept as a tombstone, not as a feature.
//
// WHAT THIS USED TO DO
//
// It answered "does this device already hold a live entitlement?" so a student
// who reinstalled could skip the code prompt. That whole mechanism is gone: it
// never worked reliably in the field, it required a device identity to be kept
// and proved on every launch, and the recovery it promised is now provided by
// the activation code itself — which the client persists to a machine-scoped
// store precisely so a reinstall does not lose it. See
// `client/src-tauri/src/locus/credential.rs`.
//
// WHY IT IS NOT SIMPLY DELETED
//
// Clients already in the field (3.2.x) call this endpoint once per launch. If
// the route vanished, PocketBase would answer 404, which a deployed client
// treats as a transport failure and logs — noise on a path that no longer
// matters. The uniform `unknown` is the answer those clients already handle
// correctly: fall through to the code prompt, which is exactly what they will
// do now.
//
// The endpoint therefore does no work: no database read, no rate limit, no
// body parsing beyond the shape check. It cannot leak, because it cannot look
// anything up.
//
// NOTE: deployed by copying to /opt/pocketbase/pb_hooks/ (see OPS.md).
// PocketBase must be restarted for a change to register.

routerAdd("POST", "/api/device-recognise", function(e) {
    // The one answer. Identical for every input, for the same reason the old
    // endpoint used a single negative: a caller learns nothing from it.
    return e.json(200, {status: "unknown", message: "This device is not recognised"});
});
