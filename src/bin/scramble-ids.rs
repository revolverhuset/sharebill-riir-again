use diesel::prelude::*;
use diesel::result::Error;
use sharebill::models::{NewCredit, NewDebit, NewTxWithId, Tx, TxItem};
use sharebill::{
    new_random_tx_id,
    schema::{credits, debits, txs},
};

fn main() {
    let src = &mut sharebill::establish_connection("test.db");
    let dst = &mut sharebill::establish_connection("test2_final2.db");

    src.transaction::<_, Error, _>(|src_c| {
        dst.transaction::<_, Error, _>(|dst_c| {
            let txs: Vec<Tx> = txs::table.load(src_c)?;

            for tx in txs {
                let new_id = new_random_tx_id(dst_c)?;

                diesel::insert_into(txs::table)
                    .values(&NewTxWithId {
                        id: new_id,
                        tx_time: tx.tx_time,
                        rev_time: tx.rev_time,
                        description: &tx.description,
                    })
                    .execute(dst_c)?;

                let items: Vec<TxItem> = credits::table
                    .select((credits::account, credits::value))
                    .filter(credits::tx_id.eq(tx.id))
                    .load(src_c)?;

                let items: Vec<_> = items
                    .iter()
                    .map(|c| NewCredit {
                        tx_id: new_id,
                        account: &c.account,
                        value: c.value.clone(),
                    })
                    .collect();

                diesel::insert_into(credits::table)
                    .values(&items)
                    .execute(dst_c)?;

                let items: Vec<TxItem> = debits::table
                    .select((debits::account, debits::value))
                    .filter(debits::tx_id.eq(tx.id))
                    .load(src_c)?;

                let items: Vec<_> = items
                    .iter()
                    .map(|c| NewDebit {
                        tx_id: new_id,
                        account: &c.account,
                        value: c.value.clone(),
                    })
                    .collect();

                diesel::insert_into(debits::table)
                    .values(&items)
                    .execute(dst_c)?;
            }

            Ok(())
        })
    })
    .expect("database error");
}
