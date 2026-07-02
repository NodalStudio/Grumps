#!/usr/bin/env bash
# replay-webhook.sh — replay correctly-authenticated webhook payloads against a
# running Grumps worker (local `wrangler dev` by default).
#
# Why this exists: the worker REJECTS any webhook whose authenticity check fails
# (WhatsApp HMAC, Telegram secret header). You cannot just POST a payload — it
# has to be signed/headed exactly like the real platform does.
#
#   Telegram  →  header X-Telegram-Bot-Api-Secret-Token == TG_WEBHOOK_SECRET
#   WhatsApp  →  header X-Hub-Signature-256 == "sha256=" + HMAC_SHA256(body, WA_APP_SECRET)
#
# Secrets are read from .dev.vars (gitignored, same file wrangler dev uses, so
# they always match). Override any of them via environment variables.
#
# Usage:
#   ./replay-webhook.sh tg "@grumps list"                 # group text message
#   ./replay-webhook.sh tg "remind me to call Bob tmrw"   # natural language
#   ./replay-webhook.sh tg "list" --dm                    # private chat (DM)
#   ./replay-webhook.sh wa "TODO: buy bread"              # WhatsApp text (needs WA_APP_SECRET)
#   ./replay-webhook.sh health                            # just hit /health
#
# Env overrides:
#   BASE          target worker URL          (default http://localhost:8787)
#   TG_CHAT_ID    Telegram group chat id     (default -100123456789)
#   TG_USER_ID    Telegram sender id         (default 111222333)
#   LANG_CODE     Telegram language_code     (default fr)
#
# Each call uses a fresh message_id (epoch-based) so the KV dedup never
# silently swallows your replay.

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

TG_WEBHOOK_SECRET="${TG_WEBHOOK_SECRET:-$(devvar TG_WEBHOOK_SECRET)}"
WA_APP_SECRET="${WA_APP_SECRET:-$(devvar WA_APP_SECRET)}"
TG_CHAT_ID="${TG_CHAT_ID:--100123456789}"
TG_USER_ID="${TG_USER_ID:-111222333}"
LANG_CODE="${LANG_CODE:-fr}"

now="$(date +%s)"

post_telegram() {
  local text="$1" mode="${2:-group}"
  local etext chat_block
  etext="$(json_escape "$text")"

  if [[ "$mode" == "dm" ]]; then
    chat_block="{\"id\": ${TG_USER_ID}, \"type\": \"private\"}"
  else
    chat_block="{\"id\": ${TG_CHAT_ID}, \"type\": \"group\", \"title\": \"Test Group\"}"
  fi

  local body
  body="{\"update_id\": ${now}, \"message\": {\"message_id\": ${now}, \"from\": {\"id\": ${TG_USER_ID}, \"first_name\": \"Alice\", \"is_bot\": false, \"language_code\": \"${LANG_CODE}\"}, \"chat\": ${chat_block}, \"date\": ${now}, \"text\": \"${etext}\"}}"

  if [[ -z "$TG_WEBHOOK_SECRET" ]]; then
    echo "!! TG_WEBHOOK_SECRET is empty — set it in .dev.vars or export it. The worker will 403." >&2
  fi

  echo "→ POST /webhook/telegram  (${mode})  text=\"${text}\""
  curl -s -X POST "$BASE/webhook/telegram" \
    -H "Content-Type: application/json" \
    -H "X-Telegram-Bot-Api-Secret-Token: ${TG_WEBHOOK_SECRET}" \
    --data-raw "$body"
  echo -e "\n"
}

post_whatsapp() {
  local text="$1"
  local etext sig body
  etext="$(json_escape "$text")"

  if [[ -z "$WA_APP_SECRET" ]]; then
    echo "!! WA_APP_SECRET not found in .dev.vars — WhatsApp HMAC cannot be computed." >&2
    echo "   Add WA_APP_SECRET=... to .dev.vars (and to the worker) to test the WA path." >&2
    return 1
  fi

  body="{\"entry\":[{\"id\":\"g1\",\"changes\":[{\"value\":{\"metadata\":{\"display_phone_number\":\"+10000000000\",\"phone_number_id\":\"123\"},\"contacts\":[{\"profile\":{\"name\":\"Alice\"}}],\"messages\":[{\"from\":\"10000000001\",\"id\":\"m${now}\",\"timestamp\":\"${now}\",\"type\":\"text\",\"text\":{\"body\":\"${etext}\"}}]}}]}]}"

  # HMAC must be computed over the EXACT bytes we send (no trailing newline).
  sig="$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$WA_APP_SECRET" | awk '{print $NF}')"

  echo "→ POST /webhook/whatsapp  text=\"${text}\""
  curl -s -X POST "$BASE/webhook/whatsapp" \
    -H "Content-Type: application/json" \
    -H "X-Hub-Signature-256: sha256=${sig}" \
    --data-raw "$body"
  echo -e "\n"
}

cmd="${1:-help}"
case "$cmd" in
  health)
    echo "→ GET /health"; curl -s "$BASE/health"; echo -e "\n" ;;
  tg)
    [[ $# -ge 2 ]] || { echo "usage: $0 tg \"<text>\" [--dm]"; exit 1; }
    mode="group"; [[ "${3:-}" == "--dm" ]] && mode="dm"
    post_telegram "$2" "$mode" ;;
  wa)
    [[ $# -ge 2 ]] || { echo "usage: $0 wa \"<text>\""; exit 1; }
    post_whatsapp "$2" ;;
  *)
    grep -E '^#( |$)' "$0" | sed 's/^# \{0,1\}//' | head -40 ;;
esac
