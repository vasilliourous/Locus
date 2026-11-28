#!/usr/bin/env bash
# Section 52 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_52() {
    echo "52. Every price in the business plan agrees with state.toml"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `docs/STATE.md` rule 2 says a fact that can be recomputed goes in `state.toml`
    # and never in a sentence. A PRICE cannot be recomputed — it is a decision — so
    # it took the other route for a year: `05-pricing.md:10-11` stated all three,
    # `11-growth.md` repeated two of them, and nothing failed when they disagreed.
    #
    # `state.toml` `[business.prices]` now holds the decision with
    # `provenance = "asserted"`, and §9 reports it without checking it (there is
    # nothing in the tree to check it against). This is the half §9 cannot do: hold
    # the PROSE to the decision, so changing a price cannot update one and not the
    # other.
    #
    # ── WHAT THIS CHECKS ──
    #
    # Each document that states a price still states the asserted one. The check is
    # per-file and per-rung, and it is deliberately tolerant of the surrounding
    # phrasing — `$2`, `$2/month`, `$2 per month` all match — because the point is
    # the NUMBER agreeing, not the sentence being frozen.
    # ─────────────────────────────────────────────────────────────
    price_report=$(cd "$REPO" && python3 - <<'PY'
import re, tomllib

prices = tomllib.load(open('docs/state.toml', 'rb'))['business']['prices']['value']
docs = ['docs/business/05-pricing.md', 'docs/business/11-growth.md']
# Which rung each document is expected to state. `11-growth.md` discusses the
# paid rung only; 05-pricing states the ladder.
expected = {
    'docs/business/05-pricing.md': ('free', 'budget', 'full'),
    'docs/business/11-growth.md':  ('full',),
}

for path in docs:
    try:
        text = open(path, encoding='utf-8').read()
    except OSError:
        print('BAD\t%s is missing — the price it states cannot be checked' % path)
        continue
    for rung in expected[path]:
        want = prices[rung]
        # The statement, not any occurrence. The FIRST draft asked "does $2
        # appear anywhere in this file", which passed with the ladder changed
        # from $2 to $3 — because `05-pricing.md` re-justifies older ladders and
        # still contained `$2` in the reasoning below the decision. A guard that
        # any historical mention satisfies is not checking the decision.
        #
        # So: find the one sentence that states this rung, and assert the number
        # in it. `Free at $0`, `Budget at $2/month`, `Full at $5/month`, and the
        # sentence may wrap across a line in the blockquote.
        # Flatten for matching, but strip the BLOCKQUOTE PREFIX FIRST: the
        # decision sentence wraps as "**Budget at\n> $2/month**", so a naive
        # `\s+` collapse leaves "at > $2" and the anchor never matches. Dropping
        # a leading `>` from each line is what makes the wrapped statement
        # readable; it changes no other text.
        flat = re.sub(r'\s+', ' ', re.sub(r'^>', '', text, flags=re.M))
        # `05-pricing.md` states the ladder normatively: "Budget at $2/month".
        m = re.search(r'\b%s\s+at\s+\$?\s*\*?\*?\s*\$\s?(\d+)' % re.escape(rung.capitalize()), flat)
        if m:
            if int(m.group(1)) != want:
                print('BAD\t%s states %s at $%s, but state.toml [business.prices] '
                      'asserts $%d' % (path, rung.capitalize(), m.group(1), want))
            continue
        # `11-growth.md` cites the price in the growth argument instead — `"$5/month"`
        # in a quoted phrase, `$5` in prose. There is no "rung at $N" sentence to
        # anchor to, so the rule is the narrower one that document actually
        # supports: every `$N` it mentions for this rung must be the asserted
        # number. Measured, not assumed: that file mentions only `$5`, twice.
        amounts = set(int(n) for n in re.findall(r'\$\s?(\d+)', flat))
        if want not in amounts:
            print('BAD\t%s mentions no $%d, and %s is the price it argues from'
                  % (path, want, rung.capitalize()))
        else:
            others = sorted(n for n in amounts if n != want)
            if others:
                print('BAD\t%s mentions %s alongside $%d; if one of those is the '
                      '%s price it now disagrees with state.toml [business.prices]'
                      % (path, ', '.join('$%d' % n for n in others), want, rung.capitalize()))
PY
    )

    if [ -z "$price_report" ]; then
        ok "the business plan states the prices asserted in state.toml [business.prices]"
    else
        while IFS="$(printf '\t')" read -r _kind message; do
            bad "prices: $message"
        done <<EOF
$price_report
EOF
    fi
}
