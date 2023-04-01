use diesel::prelude::*;

#[derive(Queryable)]
pub struct Tx {
    pub id: i32,
    pub tx_time: chrono::NaiveDateTime,
    pub rev_time: chrono::NaiveDateTime,
    pub description: String,
}

#[derive(Queryable)]
pub struct TxItem {
    pub account: String,
    pub value: i32,
}

use crate::schema::{credits, debits, txs};

#[derive(Insertable)]
#[diesel(table_name = txs)]
pub struct NewTx<'a> {
    pub tx_time: chrono::NaiveDateTime,
    pub rev_time: chrono::NaiveDateTime,
    pub description: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = credits)]
pub struct NewCredit<'a> {
    pub tx_id: i32,
    pub account: &'a str,
    pub value: i32,
}

#[derive(Insertable)]
#[diesel(table_name = debits)]
pub struct NewDebit<'a> {
    pub tx_id: i32,
    pub account: &'a str,
    pub value: i32,
}
