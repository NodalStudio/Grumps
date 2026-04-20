#!/bin/bash
# Backfill RAG vectors for existing chat history.
# Iterates workspaces, calls a worker endpoint that processes history → embeddings.
# This is one-shot, run once after Plan A deployment.
#
# NOTE: The /admin/rag-backfill endpoint is NOT YET IMPLEMENTED in the worker.
# This script is TBD — it documents the intended flow for post-launch implementation.
# When the endpoint is added, it should accept POST /admin/rag-backfill?slug=<slug>
# with Bearer token auth, read the workspace's D1 chat history, generate Vectorize
# embeddings, and upsert them into the grumps-chat-rag index.
#
# Requires: a worker endpoint POST /admin/rag-backfill?slug=... that the worker exposes.
# (We've not implemented the endpoint — this script is documented as TBD.)

set -euo pipefail

WORKER_URL="${WORKER_URL:-https://grumps-api.nodalstudio.workers.dev}"
ADMIN_TOKEN="${GRUMPS_ADMIN_TOKEN:-}"

if [ -z "$ADMIN_TOKEN" ]; then
    echo "ERROR: set GRUMPS_ADMIN_TOKEN env var"
    exit 1
fi

echo "Listing workspaces..."
WORKSPACES=$(wrangler d1 execute grumps-index \
    --command "SELECT slug FROM workspaces_meta" \
    --json --remote \
    | jq -r '.[0].results[].slug')

if [ -z "$WORKSPACES" ]; then
    echo "No workspaces found."
    exit 0
fi

echo "$WORKSPACES" | while read -r slug; do
    echo "=== Backfilling: $slug ==="
    curl -X POST "$WORKER_URL/admin/rag-backfill?slug=$slug" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -w "\nHTTP %{http_code}\n" \
        || { echo "FAILED: $slug — continuing"; }
done

echo "Done."
