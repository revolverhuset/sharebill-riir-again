CREATE TABLE txs (
    id INTEGER PRIMARY KEY NOT NULL,
    tx_time TEXT NOT NULL,
    rev_time TEXT NOT NULL,
    description TEXT NOT NULL
) STRICT;

CREATE TABLE credits (
    tx_id INTEGER REFERENCES txs (id) NOT NULL,
    account TEXT NOT NULL,
    value BLOB NOT NULL,
    PRIMARY KEY (tx_id, account)
) STRICT;

CREATE TABLE debits (
    tx_id INTEGER REFERENCES txs (id) NOT NULL,
    account TEXT NOT NULL,
    value BLOB NOT NULL,
    PRIMARY KEY (tx_id, account)
) STRICT;
