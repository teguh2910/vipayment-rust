-- Catalog service owns groups (games/apps/prepaid brands) and their products.
CREATE TABLE IF NOT EXISTS games (
    slug TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    icon_url TEXT,
    requires_zone INTEGER NOT NULL DEFAULT 0,
    user_id_label TEXT NOT NULL DEFAULT 'User ID',
    zone_label TEXT NOT NULL DEFAULT 'Zone ID',
    sort_order INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    kind TEXT NOT NULL DEFAULT 'game',
    category TEXT
);

CREATE INDEX IF NOT EXISTS idx_games_kind ON games (kind, category);
CREATE INDEX IF NOT EXISTS idx_games_active ON games (active, sort_order);

CREATE TABLE IF NOT EXISTS products (
    game_slug TEXT NOT NULL REFERENCES games(slug),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    brand TEXT,
    base_price INTEGER NOT NULL,
    sell_price INTEGER NOT NULL,
    provider_status TEXT,
    available INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (game_slug, code)
);

CREATE INDEX IF NOT EXISTS idx_products_game ON products (game_slug, available);

CREATE TABLE IF NOT EXISTS app_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
