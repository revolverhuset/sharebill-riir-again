use diesel::prelude::*;
use diesel::result::Error;
use sharebill::models::{NewCredit, NewDebit, NewTx, Tx};

fn main() {
    let conn = &mut sharebill::establish_connection();

    let now = chrono::Utc::now().naive_utc();
    let new_tx = NewTx {
        tx_time: now,
        rev_time: now,
        description: "The transaction",
    };

    conn.transaction::<_, Error, _>(|conn| {
        use sharebill::schema::{credits, debits, txs};

        let tx: Tx = diesel::insert_into(txs::table)
            .values(&new_tx)
            .get_result(conn)
            .expect("Error saving transaction");

        let new_credit = vec![NewCredit {
            tx_id: tx.id,
            account: "TBD",
            value: 5,
        }];

        let new_debit = vec![
            NewDebit {
                tx_id: tx.id,
                account: "TBD",
                value: 2,
            },
            NewDebit {
                tx_id: tx.id,
                account: "TLA",
                value: 3,
            },
        ];

        diesel::insert_into(credits::table)
            .values(&new_credit)
            .execute(conn)
            .expect("Error saving transaction");

        diesel::insert_into(debits::table)
            .values(&new_debit)
            .execute(conn)
            .expect("Error saving transaction");

        Ok(())
    })
    .expect("Error storing transaction");
}
