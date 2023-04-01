CREATE TABLE txs (
    id INTEGER PRIMARY KEY NOT NULL,
    tx_time TIMESTAMP NOT NULL,
    rev_time TIMESTAMP NOT NULL,
    description TEXT NOT NULL
);

CREATE TABLE credits (
    tx_id INTEGER REFERENCES txs (id) NOT NULL,
    account TEXT NOT NULL,
    value INTEGER NOT NULL,
    PRIMARY KEY (tx_id, account)
);

CREATE TABLE debits (
    tx_id INTEGER REFERENCES txs (id) NOT NULL,
    account TEXT NOT NULL,
    value INTEGER NOT NULL,
    PRIMARY KEY (tx_id, account)
);
