use std::fmt::Write;
use std::io;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use diesel::prelude::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    SqliteConnection,
};
use sharebill::schema::{credits, debits, txs};

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

async fn balances(pool: web::Data<DbPool>) -> actix_web::Result<impl Responder> {
    let user = web::block(move || -> Result<String, std::fmt::Error> {
        let mut conn = pool.get().expect("couldn't get db connection from pool");

        let mut result = String::new();

        let results = txs::table
            .load::<sharebill::models::Tx>(&mut conn)
            .expect("Error loading transactions");

        writeln!(&mut result, "Displaying {} transactions", results.len())?;
        for tx in results {
            let tx_time = chrono::DateTime::<chrono::Utc>::from_utc(tx.tx_time, chrono::Utc);
            writeln!(&mut result, "{} : {}", tx_time.to_rfc3339(), tx.description)?;

            writeln!(&mut result, "  Credits:")?;

            let items = credits::table
                .select((credits::account, credits::value))
                .filter(credits::tx_id.eq(tx.id))
                .load::<sharebill::models::TxItem>(&mut conn)
                .expect("Error loading transactions");

            for item in items {
                writeln!(&mut result, "    {} {}", item.account, item.value)?;
            }

            writeln!(&mut result, "  Debits:")?;

            let items = debits::table
                .select((debits::account, debits::value))
                .filter(debits::tx_id.eq(tx.id))
                .load::<sharebill::models::TxItem>(&mut conn)
                .expect("Error loading transactions");

            for item in items {
                writeln!(&mut result, "    {} {}", item.account, item.value)?;
            }
        }

        Ok(result)
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(user))
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let pool = sharebill::create_pool("test.db").expect("Could not create DB pool");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/", web::get().to(balances))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
