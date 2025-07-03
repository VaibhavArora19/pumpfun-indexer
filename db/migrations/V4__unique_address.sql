ALTER TABLE token
    ADD CONSTRAINT mint unique (mint_address);

ALTER TABLE token
    ADD CONSTRAINT bonding_curve_address unique (bonding_curve);