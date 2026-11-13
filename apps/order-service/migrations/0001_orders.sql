-- Order service owns orders, their payments, and the audit trail.
--
-- `game_*` columns denormalize the catalog group at checkout time so reading an
-- invoice never needs a call to the catalog service.
CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice TEXT NOT NULL UNIQUE,
    idempotency_key TEXT UNIQUE,
    game_slug TEXT NOT NULL,
    game_name TEXT NOT NULL DEFAULT '',
    game_icon_url TEXT,
    game_user_id_label TEXT NOT NULL DEFAULT 'User ID',
    game_zone_label TEXT NOT NULL DEFAULT 'Zone ID',
    game_category TEXT,
    game_requires_zone INTEGER NOT NULL DEFAULT 0,
    product_code TEXT NOT NULL,
    product_name TEXT NOT NULL,
    user_id TEXT NOT NULL,
    zone TEXT,
    contact TEXT NOT NULL,
    base_price INTEGER NOT NULL,
    sell_price INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    status TEXT NOT NULL,
    provider_trxid TEXT,
    provider_status TEXT,
    provider_sn TEXT,
    submit_attempts INTEGER NOT NULL DEFAULT 0,
    submission_lease_until TEXT,
    failure_reason TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    paid_at TEXT,
    submitted_at TEXT,
    completed_at TEXT,
    kind TEXT NOT NULL DEFAULT 'game'
);

CREATE INDEX IF NOT EXISTS idx_orders_status ON orders (status);
CREATE INDEX IF NOT EXISTS idx_orders_contact ON orders (contact, created_at);

CREATE TABLE IF NOT EXISTS payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id INTEGER NOT NULL REFERENCES orders(id),
    provider TEXT NOT NULL,
    method TEXT NOT NULL,
    amount INTEGER NOT NULL,
    status TEXT NOT NULL,
    proof_path TEXT,
    proof_uploaded_at TEXT,
    reviewed_by TEXT,
    reviewed_at TEXT,
    reject_reason TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_payments_status ON payments (status);
CREATE INDEX IF NOT EXISTS idx_payments_order ON payments (order_id);

CREATE TABLE IF NOT EXISTS order_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id INTEGER NOT NULL REFERENCES orders(id),
    from_status TEXT,
    to_status TEXT NOT NULL,
    note TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_order_events_order ON order_events (order_id, id);

CREATE TABLE IF NOT EXISTS app_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
