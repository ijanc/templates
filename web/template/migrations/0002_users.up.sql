CREATE TABLE users (
{%- if store == "postgres" %}
    id UUID PRIMARY KEY,
{%- else %}
    id BLOB PRIMARY KEY NOT NULL,
{%- endif %}
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    avatar_url TEXT,
    password_hash TEXT,
    provider_id TEXT,
{%- if store == "postgres" %}
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
{%- else %}
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
{%- endif %}
);

CREATE INDEX users_created_at ON users (created_at, id);
