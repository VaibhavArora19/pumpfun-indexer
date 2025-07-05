ALTER TABLE token
    ADD CONSTRAINT mint unique (contract_address);

ALTER TABLE token
    ADD CONSTRAINT bonding_curve unique (bonding_curve_address);