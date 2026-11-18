CREATE SCHEMA identity;
CREATE SCHEMA customers;
CREATE SCHEMA compliance;
CREATE SCHEMA wallets;
CREATE SCHEMA payments;
CREATE SCHEMA integrations;
CREATE SCHEMA audit;

-- numeric(38,0) would round a fractional input before a CHECK could reject it.
CREATE DOMAIN payments.minor_units AS numeric
    CHECK (VALUE >= 0 AND VALUE < 1e38::numeric AND VALUE = trunc(VALUE));

CREATE FUNCTION audit.reject_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'append-only record' USING ERRCODE = '55000';
END;
$$;
