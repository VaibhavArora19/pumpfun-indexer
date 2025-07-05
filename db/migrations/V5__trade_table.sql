CREATE TABLE IF NOT EXISTS trade (
    id uuid PRIMARY KEY,
    sol_amount bigint,
    token_amount bigint,
    is_buy boolean,
    user_address text,
    token_id uuid NOT NULL,
    FOREIGN KEY (token_id) REFERENCES token(id)
)
