-- Workspace-level locale, resolved from the bot adder's Telegram
-- language_code at first add. Used as the default for system-level
-- bot messages in the group when no member-specific locale applies.
-- NOT NULL DEFAULT 'en' so existing workspaces_meta rows populate
-- automatically at ALTER time.
ALTER TABLE workspaces_meta ADD COLUMN locale TEXT NOT NULL DEFAULT 'en';
