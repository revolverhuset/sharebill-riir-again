use std::collections::HashMap;
use std::io;
use std::{self};

use actix_web::{web, App, HttpServer, Responder};
use askama::{Template, *};
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    SqliteConnection,
};
use num::{BigInt, Zero};
use sharebill::rational::{sum_rat, Rational};
use sharebill::schema::{credits, debits, txs};

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

struct AccountBalance {
    account: String,
    balance: i64,
}

struct TransactionEntry {
    id: i32,
    when_absolute: String,
    when_relative: String,
    what: String,
    debits: Vec<Option<i64>>,
    credits: Vec<Option<i64>>,
}

struct Transactions {
    debit_accounts: Vec<String>,
    credit_accounts: Vec<String>,
    transactions: Vec<TransactionEntry>,
}

#[derive(Template)]
#[template(path = "overview.html")]
struct OverviewTemplate {
    balances: Vec<AccountBalance>,
    transactions: Transactions,
}

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate {
    id: i32,
    when: String,
    what: String,
    debits: Vec<(String, Rational)>,
    credits: Vec<(String, Rational)>,
    sum_debits: Rational,
    sum_credits: Rational,
}

async fn overview(pool: web::Data<DbPool>) -> actix_web::Result<impl Responder> {
    let pool1 = pool.clone();
    let balances = web::block(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let mut conn = pool1.get().expect("couldn't get db connection from pool");

            let cre = credits::table
                .group_by(credits::account)
                .select((credits::account, sum_rat(credits::value)))
                .load::<(String, Rational)>(&mut conn)?;
            let deb = debits::table
                .group_by(debits::account)
                .select((debits::account, sum_rat(debits::value)))
                .load::<(String, Rational)>(&mut conn)?;

            let mut balances = cre
                .into_iter()
                .map(|(account, value)| {
                    let v = value.into_inner();
                    let v = num::BigRational::from((
                        BigInt::from(v.numer().clone()),
                        BigInt::from(v.denom().clone()),
                    ));
                    (account, v)
                })
                .collect::<HashMap<_, _>>();

            for (account, value) in deb {
                let v = value.into_inner();
                let v = num::BigRational::from((
                    BigInt::from(v.numer().clone()),
                    BigInt::from(v.denom().clone()),
                ));
                *balances.entry(account).or_default() -= v;
            }

            let mut balances = balances
                .into_iter()
                .filter(|(_, balance)| !balance.is_zero())
                .map(|(account, balance)| AccountBalance {
                    account,
                    balance: balance.round().to_integer().try_into().unwrap(),
                })
                .collect::<Vec<_>>();

            balances.sort_unstable_by(|a, b| a.account.cmp(&b.account));

            Ok(balances)
        },
    );

    let transactions = web::block(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let mut conn = pool.get().expect("couldn't get db connection from pool");

            let latest_transactions = txs::table
                .order(txs::tx_time.desc())
                .limit(10)
                .load::<sharebill::models::Tx>(&mut conn)?;

            let mut debit_accounts = HashMap::<String, usize>::new();
            let mut credit_accounts = HashMap::<String, usize>::new();

            for tx in latest_transactions.iter().rev() {
                let debits = debits::table
                    .select((debits::account, debits::value))
                    .filter(debits::tx_id.eq(tx.id))
                    .load::<sharebill::models::TxItem>(&mut conn)?;

                for item in &debits {
                    debit_accounts.insert(item.account.clone(), 0);
                }

                let credits = credits::table
                    .select((credits::account, credits::value))
                    .filter(credits::tx_id.eq(tx.id))
                    .load::<sharebill::models::TxItem>(&mut conn)?;

                for item in &credits {
                    credit_accounts.insert(item.account.clone(), 0);
                }
            }

            let mut debit_account_list: Vec<_> =
                debit_accounts.keys().into_iter().cloned().collect();
            debit_account_list.sort_unstable();
            for (index, account) in debit_account_list.iter().enumerate() {
                *debit_accounts.get_mut(account).unwrap() = index;
            }

            let mut credit_account_list: Vec<_> =
                credit_accounts.keys().into_iter().cloned().collect();
            credit_account_list.sort_unstable();
            for (index, account) in credit_account_list.iter().enumerate() {
                *credit_accounts.get_mut(account).unwrap() = index;
            }

            let transactions = latest_transactions
                .iter()
                .rev()
                .map(|tx| {
                    let debits = debits::table
                        .select((debits::account, debits::value))
                        .filter(debits::tx_id.eq(tx.id))
                        .load::<sharebill::models::TxItem>(&mut conn)
                        .unwrap();

                    let mut d = vec![];
                    d.resize(debit_account_list.len(), Default::default());
                    for item in debits {
                        d[*debit_accounts.get(&item.account).unwrap()] = Some(
                            item.value
                                .into_inner()
                                .round()
                                .to_integer()
                                .try_into()
                                .unwrap(),
                        );
                    }

                    let credits = credits::table
                        .select((credits::account, credits::value))
                        .filter(credits::tx_id.eq(tx.id))
                        .load::<sharebill::models::TxItem>(&mut conn)
                        .unwrap();

                    let mut c = vec![];
                    c.resize(credit_account_list.len(), Default::default());
                    for item in credits {
                        c[*credit_accounts.get(&item.account).unwrap()] = Some(
                            item.value
                                .into_inner()
                                .round()
                                .to_integer()
                                .try_into()
                                .unwrap(),
                        );
                    }

                    let tx_time = tx.tx_time.and_local_timezone(chrono::Utc).unwrap();
                    TransactionEntry {
                        id: tx.id,
                        when_absolute: tx_time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                        when_relative: chrono_humanize::HumanTime::from(
                            tx_time.signed_duration_since(chrono::Utc::now()),
                        )
                        .to_string(),
                        what: tx.description.clone(),
                        debits: d,
                        credits: c,
                    }
                })
                .collect();

            Ok(Transactions {
                debit_accounts: debit_account_list,
                credit_accounts: credit_account_list,
                transactions,
            })
        },
    );

    let (balances, transactions) = futures::future::join(balances, transactions).await;
    let balances = balances?.map_err(actix_web::error::ErrorInternalServerError)?;
    let transactions = transactions?.map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(OverviewTemplate {
        balances,
        transactions,
    })
}

async fn post(id: web::Path<i32>, pool: web::Data<DbPool>) -> actix_web::Result<impl Responder> {
    let pool1 = pool.clone();
    let pool2 = pool.clone();
    let id = *id;

    let transaction = web::block(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let mut conn = pool.get().expect("couldn't get db connection from pool");

            let tx = txs::table
                .find(id)
                .first::<sharebill::models::Tx>(&mut conn)?;

            Ok(tx)
        },
    );

    let debits = web::block(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let mut conn = pool1.get().expect("couldn't get db connection from pool");

            let debits: Vec<(String, Rational)> = debits::table
                .select((debits::account, debits::value))
                .filter(debits::tx_id.eq(id))
                .load::<sharebill::models::TxItem>(&mut conn)?
                .into_iter()
                .map(|row| (row.account, row.value))
                .collect();

            Ok(debits)
        },
    );

    let credits = web::block(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let mut conn = pool2.get().expect("couldn't get db connection from pool");

            let credits: Vec<(String, Rational)> = credits::table
                .select((credits::account, credits::value))
                .filter(credits::tx_id.eq(id))
                .load::<sharebill::models::TxItem>(&mut conn)?
                .into_iter()
                .map(|row| (row.account, row.value))
                .collect();

            Ok(credits)
        },
    );

    let (transaction, debits, credits) = futures::future::join3(transaction, debits, credits).await;
    let debits = debits?.map_err(actix_web::error::ErrorInternalServerError)?;
    let credits = credits?.map_err(actix_web::error::ErrorInternalServerError)?;
    let transaction = transaction?.map_err(actix_web::error::ErrorInternalServerError)?;

    let sum_debits = debits.iter().map(|d| &d.1).sum();
    let sum_credits = credits.iter().map(|c| &c.1).sum();

    Ok(PostTemplate {
        id,
        what: transaction.description,
        // when: t.to_rfc3339_opts(SecondsFormat::Millis, true),
        when: chrono_humanize::HumanTime::from(
            transaction
                .tx_time
                .signed_duration_since(chrono::Local::now().naive_local()),
        )
        .to_string(),
        debits,
        credits,
        sum_debits,
        sum_credits,
    })
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let pool = sharebill::create_pool("test.db").expect("Could not create DB pool");

    HttpServer::new(move || {
        App::new()
            .service(actix_files::Files::new("/assets", "assets"))
            .app_data(web::Data::new(pool.clone()))
            .route("/", web::get().to(overview))
            .route("/post/{id}", web::get().to(post))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
