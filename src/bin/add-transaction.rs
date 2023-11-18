use diesel::prelude::*;
use diesel::result::Error;
use num::{BigInt, Zero};
use sharebill::{
    models::{NewCredit, NewDebit, NewTx, Tx},
    parse_arg::{parse_arg, EntryType},
    rational::Rational,
};

fn main() {
    let description = std::env::args()
        .nth(1)
        .expect("Usage: add-transaction \"My transaction\" ABC+1/2 XYZ-1/2");

    let args: Vec<String> = std::env::args().skip(2).map(|x| x).collect();
    let entries: Vec<(EntryType, &str, Rational)> = args
        .iter()
        .map(|x| parse_arg(&x).unwrap_or_else(|| panic!("Failed to parse argument '{}'. Should be account +/- amount, e.g. JH+5/2 or MHO-2", x)))
        .collect();

    if entries.len() == 0 {
        return;
    }
    let mut sum = num::BigRational::from((0.into(), 1.into()));
    for (entry_type, _, amount) in &entries {
        let v = amount.clone().into_inner();
        let amount = num::BigRational::from((
            BigInt::from(v.numer().clone()),
            BigInt::from(v.denom().clone()),
        ));
        match entry_type {
            EntryType::Credit => sum += amount,
            EntryType::Debit => sum -= amount,
        }
    }
    if !sum.is_zero() {
        println!("Transaction doesn't sum to zero. Aborting.");
        return;
    }

    let now = chrono::Utc::now().naive_utc();
    let new_tx = NewTx {
        tx_time: now,
        rev_time: now,
        description: &description,
    };

    let conn = &mut sharebill::establish_connection("test.db");
    conn.transaction::<_, Error, _>(|conn| {
        use sharebill::schema::{credits, debits, txs};

        let tx: Tx = diesel::insert_into(txs::table)
            .values(&new_tx)
            .get_result(conn)
            .expect("Error saving transaction");

        let mut credits = Vec::<NewCredit>::new();
        let mut debits = Vec::<NewDebit>::new();

        for entry in entries {
            match entry {
                (EntryType::Credit, account, value) => credits.push(NewCredit {
                    tx_id: tx.id,
                    account,
                    value,
                }),
                (EntryType::Debit, account, value) => debits.push(NewDebit {
                    tx_id: tx.id,
                    account,
                    value,
                }),
            }
        }

        diesel::insert_into(credits::table)
            .values(&credits)
            .execute(conn)
            .expect("Error saving transaction");

        diesel::insert_into(debits::table)
            .values(&debits)
            .execute(conn)
            .expect("Error saving transaction");

        Ok(())
    })
    .expect("Error storing transaction");
}
