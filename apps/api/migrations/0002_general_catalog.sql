ALTER TABLE games ADD COLUMN kind TEXT NOT NULL DEFAULT 'game';
ALTER TABLE games ADD COLUMN category TEXT;

ALTER TABLE orders ADD COLUMN kind TEXT NOT NULL DEFAULT 'game';

CREATE TABLE products_new (
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

INSERT INTO products_new
    (game_slug, code, name, brand, base_price, sell_price, provider_status, available, updated_at)
SELECT game_slug, code, name, NULL, base_price, sell_price, provider_status, available, updated_at
FROM products;

DROP TABLE products;
ALTER TABLE products_new RENAME TO products;

CREATE INDEX IF NOT EXISTS idx_products_game ON products (game_slug, available);
CREATE INDEX IF NOT EXISTS idx_games_kind ON games (kind, category);
