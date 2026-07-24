#!/usr/bin/env bash
# replay-waha.sh — replay a signed WAHA webhook envelope against a running
# Grumps worker (local `wrangler dev` by default).
#
# Why this exists: the worker REJECTS any /webhook/waha POST whose HMAC
# doesn't verify (crates/messaging/src/waha.rs::verify_signature). You cannot
# just POST a payload — it has to carry X-Webhook-Hmac == hex(HMAC-SHA512(raw
# body bytes, WAHA_WEBHOOK_HMAC)).
#
# The forged envelope mirrors the ground-truth shape captured from the live
# WAHA gateway (see docs/superpowers/plans/2026-07-24-waha-adapter.md):
#   me.id  = "10000000001@c.us"   (bot's own JID — placeholder number)
#   me.lid = "111111111111111@lid"
# Default target is a group chat:
#   chat JID    = "120363000000000001@g.us"
#   participant = "10000000002@c.us"          (sender — placeholder number)
# --dm sends as a direct message instead (from = participant, no participant).
#
# Usage:
#   ./replay-waha.sh "TODO: buy bread"                     # group text message
#   ./replay-waha.sh "list" --dm                            # DM
#   ./replay-waha.sh "@grumps aide" --mention                # mentionedIds path
#   ./replay-waha.sh "done" --reply-to "3EB0EE5AB4F5577CF92858"  # quoted reply
#   ./replay-waha.sh health                                  # just hit /health
#
# Env overrides:
#   BASE       target worker URL   (default http://localhost:8787)
#
# WAHA_WEBHOOK_HMAC is read from .dev.vars (same file wrangler dev uses, so
# it always matches) unless already set in the environment.
#
# Each call uses a fresh random raw-id (and epoch timestamp) so the KV dedup
# never silently swallows your replay.

set -euo pipefail
cd "$(dirname "$0")"

BASE="${BASE:-http://localhost:8787}"

# ── read a value from .dev.vars (strips KEY= and surrounding double quotes) ──
devvar() {
  local key="$1" line=""
  if [[ -f .dev.vars ]]; then
    line=$(grep -E "^${key}=" .dev.vars | head -1 || true)
    line="${line#${key}=}"
    line="${line%\"}"; line="${line#\"}"
  fi
  printf '%s' "$line"
}

# ── JSON-escape an arbitrary string (handles \ and ") ──
json_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  printf '%s' "$s"
}

WAHA_WEBHOOK_HMAC="${WAHA_WEBHOOK_HMAC:-$(devvar WAHA_WEBHOOK_HMAC)}"

ME_ID="10000000001@c.us"
ME_LID="111111111111111@lid"
GROUP_JID="120363000000000001@g.us"
PARTICIPANT="10000000002@c.us"

cmd="${1:-help}"

if [[ "$cmd" == "health" ]]; then
  echo "→ GET /health"; curl -s "$BASE/health"; echo -e "\n"
  exit 0
fi

if [[ "$cmd" == "help" || -z "$cmd" ]]; then
  grep -E '^#( |$)' "$0" | sed 's/^# \{0,1\}//' | head -40
  exit 0
fi

text="$1"; shift || true

dm=""
mention=""
reply_to_id=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dm)
      dm="1"; shift ;;
    --mention)
      mention="1"; shift ;;
    --reply-to)
      [[ $# -ge 2 ]] || { echo "usage: $0 \"<text>\" [--dm] [--mention] [--reply-to <id>]" >&2; exit 1; }
      reply_to_id="$2"; shift 2 ;;
    *)
      echo "unknown flag: $1" >&2
      echo "usage: $0 \"<text>\" [--dm] [--mention] [--reply-to <id>]" >&2
      exit 1 ;;
  esac
done

if [[ -z "$WAHA_WEBHOOK_HMAC" ]]; then
  echo "!! WAHA_WEBHOOK_HMAC not found in .dev.vars — WAHA HMAC cannot be computed." >&2
  echo "   Add WAHA_WEBHOOK_HMAC=... to .dev.vars (and to the worker) to test this path." >&2
  exit 1
fi

now="$(date +%s)"
raw_id="$(openssl rand -hex 8 | tr '[:lower:]' '[:upper:]')"
etext="$(json_escape "$text")"

if [[ -n "$dm" ]]; then
  chat_jid="$PARTICIPANT"
  participant_json="null"
else
  chat_jid="$GROUP_JID"
  participant_json="\"${PARTICIPANT}\""
fi

msg_id="false_${chat_jid}_${raw_id}"

mentioned_json="[]"
if [[ -n "$mention" ]]; then
  mentioned_json="[\"${ME_ID}\"]"
fi

reply_to_json="null"
if [[ -n "$reply_to_id" ]]; then
  ereply_id="$(json_escape "$reply_to_id")"
  reply_to_json="{\"id\": \"${ereply_id}\", \"participant\": \"${PARTICIPANT}\", \"body\": \"quoted\"}"
fi

body="{\"event\": \"message\", \"session\": \"default\", \"me\": {\"id\": \"${ME_ID}\", \"pushName\": \"Bot\", \"lid\": \"${ME_LID}\"}, \"engine\": \"NOWEB\", \"timestamp\": ${now}000, \"payload\": {\"id\": \"${msg_id}\", \"from\": \"${chat_jid}\", \"fromMe\": false, \"source\": \"app\", \"body\": \"${etext}\", \"timestamp\": ${now}, \"participant\": ${participant_json}, \"hasMedia\": false, \"media\": null, \"replyTo\": ${reply_to_json}, \"mentionedIds\": ${mentioned_json}, \"_data\": {\"pushName\": \"Test User\"}}}"

# HMAC must be computed over the EXACT bytes we send (no trailing newline).
# openssl prefixes hex digests with "(stdin)= " — strip it.
sig="$(printf '%s' "$body" | openssl dgst -sha512 -hmac "$WAHA_WEBHOOK_HMAC" | awk '{print $NF}')"

echo "→ POST /webhook/waha  (dm=${dm:-0} mention=${mention:-0} reply_to=${reply_to_id:-none})  text=\"${text}\""
response="$(curl -s -w '\n%{http_code}' -X POST "$BASE/webhook/waha" \
  -H "Content-Type: application/json" \
  -H "X-Webhook-Hmac: ${sig}" \
  -H "X-Webhook-Hmac-Algorithm: sha512" \
  --data-raw "$body")"
status="${response##*$'\n'}"
resp_body="${response%$'\n'*}"
echo "status: ${status}"
echo "body: ${resp_body}"
echo
