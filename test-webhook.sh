#!/usr/bin/env bash
# test-webhook.sh — smoke-test the WhatsApp webhook path against a local worker.
#
# The worker enforces HMAC on every WhatsApp webhook (X-Hub-Signature-256), so
# each payload below is SIGNED with WA_APP_SECRET before being sent. Earlier
# versions of this script sent a hard-coded "sha256=test" header, which the
# worker (correctly) rejects with 403 — that never exercised the real path.
#
# Requires WA_APP_SECRET in .dev.vars (the same value the worker reads in
# `wrangler dev`). If it is missing, add it to .dev.vars first.
#
# For Telegram / a more flexible replay tool, see ./replay-webhook.sh.

set -euo pipefail
cd "$(dirname "$0")"

BASE="${BASE:-http://localhost:8787}"

devvar() {
  local key="$1" line=""
  [[ -f .dev.vars ]] && line=$(grep -E "^${key}=" .dev.vars | head -1 || true)
  line="${line#${key}=}"; line="${line%\"}"; line="${line#\"}"
  printf '%s' "$line"
}

WA_APP_SECRET="${WA_APP_SECRET:-$(devvar WA_APP_SECRET)}"
if [[ -z "$WA_APP_SECRET" ]]; then
  echo "ERROR: WA_APP_SECRET not set (not in .dev.vars, not in env)." >&2
  echo "The WhatsApp webhook needs it to verify the HMAC. Add it and retry." >&2
  exit 1
fi

# Sign $1 (exact bytes) and POST it to /webhook/whatsapp.
wa() {
  local label="$1" body="$2" sig
  sig="$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$WA_APP_SECRET" | awk '{print $NF}')"
  echo "=== $label ==="
  curl -s -X POST "$BASE/webhook/whatsapp" \
    -H "Content-Type: application/json" \
    -H "X-Hub-Signature-256: sha256=${sig}" \
    --data-raw "$body"
  echo -e "\n"
}

env_msg() { # build a WhatsApp text-message webhook body
  local from="$1" id="$2" ts="$3" name="$4" text="$5"
  printf '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+10000000000","phone_number_id":"123"},"contacts":[{"profile":{"name":"%s"}}],"messages":[{"from":"%s","id":"%s","timestamp":"%s","type":"text","text":{"body":"%s"}}]}}]}]}' \
    "$name" "$from" "$id" "$ts" "$text"
}

echo "=== Health ==="
curl -s "$BASE/health"; echo -e "\n"

wa "TODO block"      "$(env_msg 10000000001 m1 1713000000 Alice 'TODO:\n• Buy bread\n• Call plumber @Bob !high')"
wa "NOTE"            "$(env_msg 10000000001 m2 1713000060 Alice 'NOTE [wifi]: password XYZ')"
wa "@grumps list"    "$(env_msg 10000000001 m3 1713000120 Alice '@grumps list')"
wa "@grumps done #1" "$(env_msg 10000000002 m4 1713000180 Bob   '@grumps done #1')"
wa "@grumps status"  "$(env_msg 10000000001 m5 1713000240 Alice '@grumps')"
wa "default create"  "$(env_msg 10000000001 m6 1713000300 Alice '@grumps book restaurant for friday @Bob')"
wa "dedup (m6 again)" "$(env_msg 10000000001 m6 1713000300 Alice '@grumps book restaurant for friday @Bob')"
echo "(the line above should be silent — dedup on message id m6)"
echo
wa "@grumps help"    "$(env_msg 10000000001 m7 1713000360 Alice '@grumps help')"
wa "DONE block"      "$(env_msg 10000000002 m8 1713000420 Bob   'DONE:\n• bread')"

echo "=== Done ==="
