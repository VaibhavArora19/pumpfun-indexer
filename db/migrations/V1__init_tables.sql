CREATE TABLE IF NOT EXISTS token (
    id uuid PRIMARY KEY,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    name text,
    ticker text,
    contract_address text,
    bonding_curve_percentage int,
    status text,
    volume bigint,
    market_cap bigint,
    holder_count int,
    fund_percentage_by_top_10 int,
    dev_holding_percentage int,
    tax int,
    image text,
    website text
);