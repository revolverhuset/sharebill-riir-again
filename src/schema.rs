diesel::table! {
    credits (tx_id, account) {
        tx_id -> Integer,
        account -> Text,
        value -> Binary,
    }
}

diesel::table! {
    debits (tx_id, account) {
        tx_id -> Integer,
        account -> Text,
        value -> Binary,
    }
}

diesel::table! {
    txs (id) {
        id -> Integer,
        tx_time -> Timestamp,
        rev_time -> Timestamp,
        description -> Text,
    }
}

diesel::joinable!(credits -> txs (tx_id));
diesel::joinable!(debits -> txs (tx_id));

diesel::allow_tables_to_appear_in_same_query!(credits, debits, txs,);
