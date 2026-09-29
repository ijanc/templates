CREATE TABLE items (
{%- if store == "postgres" %}
    id UUID PRIMARY KEY,
{%- else %}
    id BLOB PRIMARY KEY NOT NULL,
{%- endif %}
    name TEXT NOT NULL,
    description TEXT,
{%- if store == "postgres" %}
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
{%- else %}
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
{%- endif %}
);

CREATE INDEX items_created_at ON items (created_at, id);
