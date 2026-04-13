#!/bin/bash
# test-webhook.sh — send test webhook payloads to the local worker
BASE="http://localhost:8787"

echo "=== Health ==="
curl -s "$BASE/health"
echo -e "\n"

echo "=== TODO block ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m1","timestamp":"1713000000","type":"text","text":{"body":"TODO:\n\u2022 Buy bread\n\u2022 Call plumber @Bob !high"}}]}}]}]}'
echo -e "\n"

echo "=== NOTE ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m2","timestamp":"1713000060","type":"text","text":{"body":"NOTE [wifi]: password XYZ"}}]}}]}]}'
echo -e "\n"

echo "=== @grumps list ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m3","timestamp":"1713000120","type":"text","text":{"body":"@grumps list"}}]}}]}]}'
echo -e "\n"

echo "=== @grumps done #1 ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Bob"}}],"messages":[{"from":"337","id":"m4","timestamp":"1713000180","type":"text","text":{"body":"@grumps done #1"}}]}}]}]}'
echo -e "\n"

echo "=== @grumps (status) ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m5","timestamp":"1713000240","type":"text","text":{"body":"@grumps"}}]}}]}]}'
echo -e "\n"

echo "=== Default: create todo ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m6","timestamp":"1713000300","type":"text","text":{"body":"@grumps book restaurant for friday @Bob"}}]}}]}]}'
echo -e "\n"

echo "=== Dedup (m6 again) ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m6","timestamp":"1713000300","type":"text","text":{"body":"@grumps book restaurant for friday @Bob"}}]}}]}]}'
echo "(should be silent — dedup)"
echo -e "\n"

echo "=== @grumps help ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m7","timestamp":"1713000360","type":"text","text":{"body":"@grumps help"}}]}}]}]}'
echo -e "\n"

echo "=== DONE block ==="
curl -s -X POST "$BASE/webhook/whatsapp" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=test" \
  -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Bob"}}],"messages":[{"from":"337","id":"m8","timestamp":"1713000420","type":"text","text":{"body":"DONE:\n\u2022 bread"}}]}}]}]}'
echo -e "\n"

echo "=== Done ==="
