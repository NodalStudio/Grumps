#!/bin/bash
# Apply migrations 0002/0003/0004 to all existing workspaces.
# Idempotent: safe to re-run.
# See spec § 15.2.

set -euo pipefail

echo "Listing workspaces from Index DB..."
WORKSPACES=$(wrangler d1 execute grumps-index \
    --command "SELECT slug, d1_database_id FROM workspaces_meta" \
    --json --remote \
    | jq -r '.[0].results[] | "\(.d1_database_id)\t\(.slug)"')

if [ -z "$WORKSPACES" ]; then
    echo "No workspaces found. Nothing to migrate."
    exit 0
fi

echo "$WORKSPACES" | while IFS=$'\t' read -r db_id slug; do
    echo ""
    echo "=== Migrating workspace: $slug ($db_id) ==="
    for mig in 0002_memory 0003_calendar 0004_scheduling 0005_migrate_reminders 0007_quality_signals; do
        echo "  Applying $mig.sql..."
        wrangler d1 execute "$db_id" \
            --file="migrations/workspace/${mig}.sql" \
            --remote \
            || { echo "  FAILED: $slug/$mig — STOPPING"; exit 1; }
    done
    echo "  ✓ Done: $slug"
done

echo ""
echo "All workspaces migrated successfully."
