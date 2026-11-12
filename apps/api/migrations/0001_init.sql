CREATE TABLE IF NOT EXISTS games (
    slug TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    icon_url TEXT,
    requires_zone INTEGER NOT NULL DEFAULT 0,
    user_id_label TEXT NOT NULL DEFAULT 'User ID',
    zone_label TEXT NOT NULL DEFAULT 'Zone ID',
    sort_order INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS products (
    code TEXT PRIMARY KEY,
    game_slug TEXT NOT NULL REFERENCES games(slug),
    name TEXT NOT NULL,
    base_price INTEGER NOT NULL,
    sell_price INTEGER NOT NULL,
    provider_status TEXT,
    available INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_products_game ON products (game_slug, available);

CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice TEXT NOT NULL UNIQUE,
    idempotency_key TEXT UNIQUE,
    game_slug TEXT NOT NULL,
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
    completed_at TEXT
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
