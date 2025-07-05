ALTER TABLE trade
    ADD COLUMN created_at timestamptz NOT NULL;

ALTER TABLE trade
    ADD COLUMN updated_at timestamptz NOT NULL;