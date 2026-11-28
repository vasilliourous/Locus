# Runbook: "the hub says signed-out, the client says logged-in"

```
audience:    operator
status:      live
authoritative-for: the ordered diagnosis of a hub/client auth desync
verified-against: docs/reference/API.md §5d, docs/reference/AUTH-STATE.md,
                  server/pb_hooks/session_probe.pb.js
```

You have a report of the form *"I'm logged in but the console says I'm signed
out"*. Work down this page in order. **Do not skip to step 4** — steps 1–3 cost
nothing and eliminate most reports.

---

## 0. Before you ask the student anything

Look at the Codes page. Find the code. There are now **two** columns where there
used to be one:

| Column | Answers | Owned by |
|---|---|---|
| **State** | may this code be used | the hub (entitlement) |
| **Last seen** | is the device checking in | the client (liveness) |

> These two columns answer different questions and can look like they disagree
> without anything being wrong. **Read them as a pair, never as one verdict.**

> **If State says "unclaimed" and Last seen says "checked in 2m ago",
> the device is fine and the claim is stale.** That is D3 in
> [`../reference/AUTH-STATE.md`](../reference/AUTH-STATE.md) §4, and it is the
> most common form of this report. Go to step 4.
>
> The wording is deliberate: **"unclaimed" is a fact about the code**
> ("a Sign out was recorded since the last activation, and nobody has claimed it"),
> not about the device. It is the same label the console's State column shows; it
> was shortened from "sign-out recorded" so it fits that column without clipping.
> It used to read "signed out", which invited exactly the inference that caused
> this report — an operator seeing that word beside "checked in 2m ago" believes
> the word, and the word was never about the device.

> **If Last seen shows `—`, nothing is known.** That is not "signed out" and not
> "gone". It means the code has not beaten since the liveness column existed.
> Treat it as unknown and continue.

---

## 1. Is the hub the one you think it is? (30 seconds)

The hooks on the box are deployed from its own copy, not from this repository
(`hooks-sync.sh`). If `LAST SEEN` is `—` for **every** code, the hub is probably
not writing the column at all:

```sh
ssh <hub> 'sudo /root/server/scripts/hooks-sync.sh --check'
```

* **Differs** → the hub is running older hooks. Redeploy (`hooks-sync.sh`), then
  re-check. This is the "a repo fix is inert until the piece that runs it is
  redeployed" trap and it is the most likely cause of a *fleet-wide* `—`.
* **Matches** → continue.

Also confirm the schema has the column:

```sh
ssh <hub> 'sqlite3 /root/pocketbase/pb_data/data.db \
  "SELECT count(*) FROM _collections WHERE name=\"codes\"" \
  2>/dev/null || true'
```

If `seed-pb.py` has the column but the live collection does not, the schema step
was not re-run. **Stop and re-run it** — until then every beat throws into the
guard's `catch` and liveness is silently always unknown.

---

## 2. Ask the student for three things

One message, all three, so there is no second round trip:

1. **The code**, exactly as shown on their Account screen.
2. **What the app says** — the Account screen's status line, verbatim.
3. **Whether it connects.** "Does the VPN actually turn on?"

---

## 3. Run the probe

Ask them to run it from the app's Account screen (it needs the device's own
fingerprint), or run it yourself if you have both the code and their device id:

```sh
curl -sS -X POST https://<hub>/api/session-probe \
  -H 'content-type: application/json' \
  -d '{
    "probe_version": 1,
    "code": "RQ-ABCD-EFGH-JKMN-T",
    "device_id": "a1b2c3d4e5f6",
    "client": { "auth_state": "activated", "app_version": "3.5.2", "platform": "linux" }
  }' | python3 -m json.tool
```

**A bare `404` means one of:** unknown code, suspended, expired, or a `device_id`
that is not the machine that first redeemed it. It does **not** mean "the code
does not exist" — the endpoint is deliberately not an existence oracle. If you
get a bare 404 and expected data, check the `device_id` against
`first_seen_fingerprint` in the console before concluding anything.

### 3a. What the probe does when the network blocks the hub

**The probe tries the free tunnel by itself, every time.** This matters because
the probe is most needed on exactly the networks that refuse the hub by name —
and a diagnostic that fails there would report "the hub is down", which is the
wrong diagnosis.

Concretely: it attempts the direct route, and on a transport failure
(reset / refused / DNS / **silent timeout**) it retries once through the free
tunnel, starting one if none is running. The two outcomes read differently and
you should treat them differently:

| The probe returns | What it means |
|---|---|
| a result | The device reaches the hub (possibly through the free tunnel — the app logs which) |
| "could not be reached directly … the free tunnel failed too" | Their network blocks **both**. This is a network conversation, not a hub problem. |
| "this build carries no free-tier key" | An old or stripped build. Check the version before blaming the network. |
| "the hub did not answer: … certificate" | The hub **was** reached; this is a clock or certificate-store problem on the device. |

> **The free tunnel is not always running.** It is deliberately stopped once the
> plan tunnel is proven (`TUNNEL-SUPERVISOR.md` §3), so a device that is happily
> connected has no free-tunnel engine warm. The probe handles that — it starts
> one — which is why a probe may take several seconds the first time on a
> blocked network. That delay is the engine starting, not the hub hanging.
>
> The exception is a device that carries **no free-tier key**, where no engine can
> be started at all. The probe says so explicitly rather than pretending the
> network is at fault.

---

## 4. Read `reason_code`, and do exactly what the table says

| `reason_code` | What it means | What you do |
|---|---|---|
| `client_beating_hub_quiet` | The device is checking in. The console's "signed out" was an inference from `released_at`. | **Nothing on the device.** If State also says signed out, treat it as D3 below. |
| `stale_release_claim` | A sign-out claim contradicts a device that is demonstrably still using the code. | Ask the student to sign in once — that spends the claim. If they cannot, `codes.unbind` from the console. **Do not** reset the term; unbind only clears `activated_at`. |
| `hub_refused_client_hasnt_noticed` | The hub already refuses (suspended or expired); the client has not beaten since. | Wait one beat, or press "check now". If it persists past two beats, it is a client problem — get the Account screen. |
| `client_unreachable` | No recent beat. Killed, offline, or quit — **the probe does not know which**. | Nothing, unless the student is on the phone. If they are, step 5. |
| `client_dropped_session` | The client lost its own record; the hub still holds a live one. | Student re-enters the code. Nothing is orphaned — a code is single-use, not device-bound. |
| `genuinely_signed_out` | Both sides agree. | Nothing. |
| `in_sync` | The hub side is healthy and **the client did not report**, so the other half is unobserved. | Nothing on the hub. If the report stands, reach the student (step 5) — the probe could not see their half. |
| `clock_skew` | The two clocks differ by >300 s. **Reported, never acted on.** | Nothing. Liveness uses the hub's clock, so this cannot cause a wrong answer — it only explains a strange-looking client. |
| `unknown` | Nothing to conclude. | Use the console, and check step 1 again. |

---

## 5. The student is on the phone and `client_unreachable`

Ask them to note what the Account screen says for **"last confirmed"**. That is
the client's own `last_heartbeat_ok`, and comparing it to now tells you which
case this is:

| Client says | Means |
|---|---|
| confirmed minutes ago, but the hub has no recent beat | The client reaches the hub and the hub is not recording it — **step 1 again**, this is a deployment problem |
| confirmed hours/days ago, `heartbeat_failures` high | The client cannot reach the hub. Ask about their network — a school that blocks the hostname is the known case, and the app has a free tunnel for it. **The probe tries that tunnel automatically** (see §3a), so a probe that still fails is evidence their network blocks both. |
| never confirmed | They have never had a successful beat. Check they actually completed activation |

---

## 6. What NOT to do

* **Do not reset or shorten the term** to "fix" a display problem. `codes.unbind`
  clears the single-use stamp; it does not change `expires_at`. A renewal is a
  payment event and nothing here is a payment event.
* **Do not edit the database by hand.** Every state in §4 is reachable through
  `codes.unbind` (audited) or by waiting one lease (automatic). If you are about
  to `UPDATE`, stop — you have found a gap in this runbook, and the gap is the
  bug to fix.
* **Do not assume `—` in Last seen means the device is gone.** It means nobody
  knows.

---

## 7. When it is the console's fault, say so

If `reason_code` is `client_beating_hub_quiet` or `stale_release_claim`, **the
device is healthy and the display was wrong.** Record that on the ticket. The
distinction matters beyond the one student: a support load built from console
defects looks identical to one built from real failures, and only the ticket
text separates them.
