use diesel::prelude::*;
use diesel::result::Error;
use num::bigint::ToBigUint;
use regex::Regex;
use serde::{
    de::{self, MapAccess, Unexpected, Visitor},
    Deserializer,
};
use sharebill::{
    models::{NewCredit, NewDebit, NewTx, Tx},
    rational::Rational,
};
use std::sync::OnceLock;
use std::{collections::BTreeMap, fmt, marker::PhantomData};

struct RationalVisitor;

pub fn parse_mixed_number(number: &str) -> Result<Rational, String> {
    use std::str::FromStr;

    static MIXED_NUMBER: OnceLock<Regex> = OnceLock::new();

    let mixed_number =
        MIXED_NUMBER.get_or_init(|| Regex::new(r"^((-)?(\d+)( (\d+/\d+))?|(-?\d+/\d+))$").unwrap());

    match mixed_number.captures(number) {
        Some(groups) => {
            let mut result = Rational::from_str("0").unwrap();
            if let Some(x) = groups.get(3) {
                result = result + Rational::from_str(x.as_str()).unwrap();
            }
            if let Some(x) = groups.get(5) {
                result = result + Rational::from_str(x.as_str()).unwrap();
            }
            if let Some(x) = groups.get(6) {
                result = result + Rational::from_str(x.as_str()).unwrap();
            }
            if let Some(_) = groups.get(2) {
                // result = -result;
                return Err("Cannot import negative numbers".to_owned());
            }
            Ok(result)
        }
        None => Err("Not a valid mixed number".to_string()),
    }
}

impl<'de> Visitor<'de> for RationalVisitor {
    type Value = Rational;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a number or string containing a rational number")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        parse_mixed_number(s).map_err(|_| de::Error::invalid_value(Unexpected::Str(s), &self))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Rational::new(
            v.to_biguint()
                .ok_or_else(|| de::Error::invalid_value(Unexpected::Signed(v), &self))?,
            1u32,
        ))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Rational::new(v, 1u32))
    }
}

fn deserialize_rational<'de, D>(de: D) -> Result<Rational, D::Error>
where
    D: Deserializer<'de>,
{
    de.deserialize_any(RationalVisitor)
}

// fn serialize_rational<S>(x: &Rational, s: S) -> Result<S::Ok, S::Error>
// where
//     S: Serializer,
// {
//     s.serialize_str(&format!("{}/{}", x.numer(), x.denom()))
// }

struct MyMapVisitor {
    marker: PhantomData<fn() -> BTreeMap<String, Rational>>,
}

impl MyMapVisitor {
    fn new() -> Self {
        MyMapVisitor {
            marker: PhantomData,
        }
    }
}

impl<'de> Visitor<'de> for MyMapVisitor {
    // The type that our Visitor is going to produce.
    type Value = Vec<(String, Rational)>;

    // Format a message stating what data this Visitor expects to receive.
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a very special map")
    }

    // Deserialize MyMap from an abstract "map" provided by the
    // Deserializer. The MapAccess input is a callback provided by
    // the Deserializer to let us see each entry in the map.
    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        #[derive(serde_derive::Deserialize, Debug)]
        struct T(#[serde(deserialize_with = "deserialize_rational")] Rational);

        let mut v = Vec::with_capacity(access.size_hint().unwrap_or(0));

        // While there are entries remaining in the input, add them
        // into our map.
        while let Some((key, value)) = access.next_entry()? {
            let value: T = value;
            v.push((key, value.0));
        }

        Ok(v)
    }
}

fn deserialize_mymap<'de, D>(de: D) -> Result<Vec<(String, Rational)>, D::Error>
where
    D: Deserializer<'de>,
{
    de.deserialize_map(MyMapVisitor::new())
}

#[derive(serde_derive::Deserialize, Debug)]
struct Transaction {
    // deserialize_with et eller annet!!!
    #[serde(deserialize_with = "deserialize_mymap")]
    credits: Vec<(String, Rational)>,
    // deserialize_with et eller annet!!!
    #[serde(deserialize_with = "deserialize_mymap", rename = "debets")]
    debits: Vec<(String, Rational)>,
}

#[derive(serde_derive::Deserialize, Debug)]
struct Meta {
    timestamp: chrono::DateTime<chrono::Utc>,
    description: String,
}

#[derive(serde_derive::Deserialize, Debug)]
struct TransactionDocument {
    // _id: String,
    // _rev: String,
    transaction: Transaction,
    meta: Meta,
}

#[derive(serde_derive::Deserialize, Debug)]
struct Row {
    // id: String,
    // key: String,
    value: TransactionDocument,
}

#[derive(serde_derive::Deserialize, Debug)]
struct AllDocs {
    // total_rows: u32,
    // offset: u32,
    rows: Vec<Row>,
}

fn main() {
    // 1. Input JSON from somewhere (stdin? file?)
    let input = std::io::stdin().lock();

    // 2. Parse the heck out of it
    let docs: AllDocs = serde_json::from_reader(input).unwrap();

    println!("{docs:?}");

    // 3. Shove it into the database
    let conn = &mut sharebill::establish_connection();

    conn.transaction::<_, Error, _>(|conn| {
        use sharebill::schema::{credits, debits, txs};

        for row in docs.rows {
            let meta = row.value.meta;
            let transaction = row.value.transaction;

            let new_tx = NewTx {
                tx_time: meta.timestamp.naive_utc(),
                rev_time: meta.timestamp.naive_utc(),
                description: &meta.description,
            };

            let tx: Tx = diesel::insert_into(txs::table)
                .values(&new_tx)
                .get_result(conn)
                .expect("Error saving transaction");

            let new_credits: Vec<NewCredit> = transaction
                .credits
                .iter()
                .map(|credit| NewCredit {
                    tx_id: tx.id,
                    account: &credit.0,
                    value: credit.1.clone(),
                })
                .collect();

            diesel::insert_into(credits::table)
                .values(&new_credits)
                .execute(conn)
                .expect("Error saving transaction");

            let new_debits: Vec<NewDebit> = transaction
                .debits
                .iter()
                .map(|debit| NewDebit {
                    tx_id: tx.id,
                    account: &debit.0,
                    value: debit.1.clone(),
                })
                .collect();

            diesel::insert_into(debits::table)
                .values(&new_debits)
                .execute(conn)
                .expect("Error saving transaction");
        }

        Ok(())
    })
    .expect("Error storing transaction");
}
