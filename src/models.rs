use diesel::prelude::*;
use id30::Id30;

#[derive(Queryable)]
pub struct Tx {
    pub id: Id30,
    pub tx_time: chrono::NaiveDateTime,
    pub rev_time: chrono::NaiveDateTime,
    pub description: String,
}

#[derive(Queryable)]
pub struct TxItem {
    pub account: String,
    pub value: Rational,
}

use crate::{
    rational::Rational,
    schema::{credits, debits, txs},
};

#[derive(Insertable)]
#[diesel(table_name = txs)]
pub struct NewTx<'a> {
    pub tx_time: chrono::NaiveDateTime,
    pub rev_time: chrono::NaiveDateTime,
    pub description: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = txs)]
pub struct NewTxWithId<'a> {
    pub id: Id30,
    pub tx_time: chrono::NaiveDateTime,
    pub rev_time: chrono::NaiveDateTime,
    pub description: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = credits)]
pub struct NewCredit<'a> {
    pub tx_id: Id30,
    pub account: &'a str,
    pub value: Rational,
}

#[derive(Insertable)]
#[diesel(table_name = debits)]
pub struct NewDebit<'a> {
    pub tx_id: Id30,
    pub account: &'a str,
    pub value: Rational,
}
