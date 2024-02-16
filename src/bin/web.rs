use std::collections::HashMap;
use std::io;
use std::{self};

use actix_web::web::Redirect;
use actix_web::{web, App, HttpServer, Responder, ResponseError};
use askama::{Template, *};
use chrono::{DateTime, SecondsFormat, Utc};
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    SqliteConnection,
};
use num::{BigInt, Zero};
use serde::de::Error;
use serde_derive::Deserialize;
use sharebill::models::{NewCredit, NewDebit, NewTxWithId};
use sharebill::rational::{sum_rat, Rational, RationalVisitor};
use sharebill::schema::{credits, debits, txs};
use thiserror::Error;

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

async fn get_transaction(
    id: web::Path<i32>,
    pool: web::Data<DbPool>,
) -> actix_web::Result<impl Responder> {
    let pool1 = pool.clone();
    let pool2 = pool.clone();
    let id = *id;

    let transaction = web::block(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let mut conn = pool.get().expect("couldn't get db connection from pool");

            let tx = txs::table
                .find(id)
                .first::<sharebill::models::Tx>(&mut conn)
                .optional()?;

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
    let mut debits = debits?.map_err(actix_web::error::ErrorInternalServerError)?;
    let mut credits = credits?.map_err(actix_web::error::ErrorInternalServerError)?;
    let transaction = transaction?
        .map_err(actix_web::error::ErrorInternalServerError)?
        .unwrap_or_else(|| sharebill::models::Tx {
            id,
            tx_time: chrono::Utc::now().naive_utc(),
            rev_time: chrono::Utc::now().naive_utc(),
            description: String::new(),
        });

    let sum_debits = debits.iter().map(|d| &d.1).sum();
    let sum_credits = credits.iter().map(|c| &c.1).sum();

    let rows = std::cmp::max(std::cmp::max(debits.len(), credits.len()) + 3, 5);
    debits.resize(rows, Default::default());
    credits.resize(rows, Default::default());

    Ok(PostTemplate {
        id,
        what: transaction.description,
        when: transaction
            .tx_time
            .and_local_timezone(chrono::Utc)
            .unwrap()
            .to_rfc3339_opts(SecondsFormat::Millis, true),
        debits,
        credits,
        sum_debits,
        sum_credits,
    })
}

struct TransactionItemsVisitor {
    key_field: &'static str,
    value_field: &'static str,
}

impl<'de> serde::de::Visitor<'de> for TransactionItemsVisitor {
    type Value = HashMap<String, Rational>;

    fn expecting(&self, _formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        todo!()
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut keys = vec![];
        let mut values = vec![];

        while let Some((key, value)) = map.next_entry::<String, String>()? {
            if key == self.key_field {
                keys.push(value);
            } else if key == self.value_field {
                if value.is_empty() && keys.last().map(|key| key.is_empty()).unwrap_or(false) {
                    keys.pop();
                    // ignore this value
                } else {
                    values.push(value.parse().map_err(|_| {
                        A::Error::invalid_value(
                            serde::de::Unexpected::Str(&value),
                            &RationalVisitor,
                        )
                    })?);
                }
            }
        }

        Ok(keys.into_iter().zip(values).collect())
    }
}

fn deserialize_debits<'de, D>(deserializer: D) -> Result<HashMap<String, Rational>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_map(TransactionItemsVisitor {
        key_field: "debit_account",
        value_field: "debit_value",
    })
}

fn deserialize_credits<'de, D>(deserializer: D) -> Result<HashMap<String, Rational>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_map(TransactionItemsVisitor {
        key_field: "credit_account",
        value_field: "credit_value",
    })
}

#[derive(Debug, Deserialize)]
struct InsertTransaction {
    when: DateTime<Utc>,
    what: String,

    #[serde(flatten, deserialize_with = "deserialize_debits")]
    debits: HashMap<String, Rational>,

    #[serde(flatten, deserialize_with = "deserialize_credits")]
    credits: HashMap<String, Rational>,
}

#[derive(Error, Debug)]
enum ValidationError {
    #[error("no transaction, credits and debits are zero")]
    ZeroValue,
    #[error("missing description")]
    MissingDescription,
    #[error("unbalanced transaction, credits != debits")]
    Unbalanced,
    #[error("empty account name")]
    EmptyAccountName,
}

impl ResponseError for ValidationError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::BAD_REQUEST
    }
}

impl InsertTransaction {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.what.is_empty() {
            return Err(ValidationError::MissingDescription);
        }

        let sum_debits: Rational = self.debits.values().sum();
        let sum_credits: Rational = self.credits.values().sum();
        if sum_debits != sum_credits {
            return Err(ValidationError::Unbalanced);
        }
        if sum_debits.is_zero() {
            return Err(ValidationError::ZeroValue);
        }

        if self.credits.keys().any(|account| account.is_empty())
            || self.debits.keys().any(|account| account.is_empty())
        {
            return Err(ValidationError::EmptyAccountName);
        }

        Ok(())
    }
}

async fn post_transaction(
    id: web::Path<i32>,
    pool: web::Data<DbPool>,
    web::Form(doc): web::Form<InsertTransaction>,
) -> actix_web::Result<impl Responder> {
    // 1. Validate `doc`
    doc.validate()?;

    // 2. In a database transaction:
    let mut conn = pool.get().expect("couldn't get db connection from pool");
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // 2a. Delete from credits and debits where tx_id=_id, delete from txs where id=_id
        // 2b. Insert new transaction, like in add-transaction

        diesel::delete(credits::table.filter(credits::tx_id.eq(*id))).execute(conn)?;
        diesel::delete(debits::table.filter(debits::tx_id.eq(*id))).execute(conn)?;
        diesel::delete(txs::table.filter(txs::id.eq(*id))).execute(conn)?;

        diesel::insert_into(txs::table)
            .values(&NewTxWithId {
                id: *id,
                tx_time: doc.when.naive_utc(),
                rev_time: chrono::Utc::now().naive_utc(),
                description: &doc.what,
            })
            .execute(conn)?;

        diesel::insert_into(credits::table)
            .values(
                doc.credits
                    .iter()
                    .map(|(account, value)| NewCredit {
                        tx_id: *id,
                        account,
                        value: value.clone(),
                    })
                    .collect::<Vec<NewCredit>>(),
            )
            .execute(conn)?;

        diesel::insert_into(debits::table)
            .values(
                doc.debits
                    .iter()
                    .map(|(account, value)| NewDebit {
                        tx_id: *id,
                        account,
                        value: value.clone(),
                    })
                    .collect::<Vec<NewDebit>>(),
            )
            .execute(conn)?;

        Ok(())
    })
    .map_err(actix_web::error::ErrorInternalServerError)?;

    // ON SUCCESS redirect to GET of the same URL
    Ok(Redirect::to("").see_other())
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let pool = sharebill::create_pool("test.db").expect("Could not create DB pool");

    HttpServer::new(move || {
        App::new()
            .service(actix_files::Files::new("/assets", "assets"))
            .app_data(web::Data::new(pool.clone()))
            .route("/", web::get().to(overview))
            .route("/post/{id}", web::get().to(get_transaction))
            .route("/post/{id}", web::post().to(post_transaction))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
