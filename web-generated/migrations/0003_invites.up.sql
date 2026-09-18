CREATE TABLE invites (
    token TEXT PRIMARY KEY NOT NULL,
    email TEXT,
    created_by BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    used_by BLOB REFERENCES users (id) ON DELETE SET NULL
);

CREATE INDEX invites_created_at ON invites (created_at, token);
