CREATE TABLE invites (
    token TEXT PRIMARY KEY NOT NULL,
    email TEXT,
{%- if store == "postgres" %}
    created_by UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    used_by UUID REFERENCES users (id) ON DELETE SET NULL
{%- else %}
    created_by BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    used_by BLOB REFERENCES users (id) ON DELETE SET NULL
{%- endif %}
);

CREATE INDEX invites_created_at ON invites (created_at, token);
