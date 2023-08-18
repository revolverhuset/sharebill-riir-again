use diesel::prelude::*;
use sharebill::schema::{credits, debits, txs};

fn main() {
    let conn = &mut sharebill::establish_connection("test.db");

    let results = txs::table
        .load::<sharebill::models::Tx>(conn)
        .expect("Error loading transactions");

    println!("Displaying {} transactions", results.len());
    for tx in results {
        let tx_time = chrono::DateTime::<chrono::Utc>::from_utc(tx.tx_time, chrono::Utc);
        println!("{} : {}", tx_time.to_rfc3339(), tx.description);

        println!("  Credits:");

        let items = credits::table
            .select((credits::account, credits::value))
            .filter(credits::tx_id.eq(tx.id))
            .load::<sharebill::models::TxItem>(conn)
            .expect("Error loading transactions");

        for item in items {
            println!("    {} {}", item.account, item.value);
        }

        println!("  Debits:");

        let items = debits::table
            .select((debits::account, debits::value))
            .filter(debits::tx_id.eq(tx.id))
            .load::<sharebill::models::TxItem>(conn)
            .expect("Error loading transactions");

        for item in items {
            println!("    {} {}", item.account, item.value);
        }
    }
}
