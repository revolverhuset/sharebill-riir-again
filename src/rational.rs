use diesel::deserialize::{self, FromSql};
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Binary;
use diesel::sqlite::{Sqlite, SqliteAggregateFunction, SqliteValue};
use diesel::{AsExpression, FromSqlRow};
use num::rational::Ratio;
use num::{BigUint, Zero};

#[derive(Default, PartialEq, Eq, Debug, AsExpression, FromSqlRow, Clone)]
#[diesel(sql_type = Binary)]
pub struct Rational(Ratio<BigUint>);

sql_function! {
    #[aggregate]
    fn sum_rat(x: Binary) -> Binary;
}

#[derive(Default)]
pub struct SumRat {
    sum: Rational,
}

impl SqliteAggregateFunction<Rational> for SumRat {
    type Output = Rational;

    fn step(&mut self, expr: Rational) {
        self.sum += expr;
    }

    fn finalize(aggregator: Option<Self>) -> Self::Output {
        aggregator.map(|a| a.sum).unwrap_or_default()
    }
}

impl Rational {
    pub fn new(numer: impl Into<BigUint>, denom: impl Into<BigUint>) -> Self {
        Self(Ratio::new(numer.into(), denom.into()))
    }

    pub fn into_inner(self) -> Ratio<BigUint> {
        self.0
    }
}

impl std::str::FromStr for Rational {
    type Err = <Ratio<BigUint> as std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ratio::<BigUint>::from_str(s).map(|r| Rational(r))
    }
}

impl std::ops::Add for Rational {
    type Output = Rational;

    fn add(self, rhs: Rational) -> Self::Output {
        Rational(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for Rational {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl From<u32> for Rational {
    fn from(value: u32) -> Self {
        Self::new(value, 1u32)
    }
}

impl std::fmt::Display for Rational {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ToSql<Binary, Sqlite> for Rational {
    fn to_sql<'c>(&'c self, out: &mut Output<'c, '_, Sqlite>) -> serialize::Result {
        let numer = self.0.numer().to_bytes_le();
        let denom = self.0.denom().to_bytes_le();

        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&(numer.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&numer);
        bytes.extend_from_slice(&denom);

        out.set_value(bytes);
        Ok(serialize::IsNull::No)
    }
}

#[derive(Debug)]
struct ParseError;

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid rational number")
    }
}

impl std::error::Error for ParseError {}

// Not (yet?) in the standard library.
// From https://internals.rust-lang.org/t/slice-split-at-should-have-an-option-variant/17891
#[inline]
fn get_split_at(slice: &[u8], mid: usize) -> Option<(&[u8], &[u8])> {
    if mid > slice.len() {
        None
    } else {
        Some(slice.split_at(mid))
    }
}

impl FromSql<Binary, Sqlite> for Rational {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let bytes = <*const [u8] as FromSql<Binary, Sqlite>>::from_sql(bytes)?;
        let bytes: &[u8] = unsafe { &*bytes };

        let (header, values) = get_split_at(bytes, 4).ok_or(ParseError)?;
        let numer_len = u32::from_le_bytes(header.try_into().unwrap()) as usize;

        let (numer, denom) = get_split_at(values, numer_len).ok_or(ParseError)?;

        let numer = BigUint::from_bytes_le(numer);
        let denom = BigUint::from_bytes_le(denom);

        if denom.is_zero() {
            return Err(ParseError.into());
        }

        Ok(Rational(Ratio::new(numer, denom)))
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;

    use diesel::prelude::*;
    use diesel::sql_query;
    use diesel::sql_types::Binary;

    use super::*;

    #[test]
    fn basic_db_roundtrip() -> Result<(), Box<dyn Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;

        #[derive(QueryableByName, PartialEq, Eq, Debug)]
        struct Row {
            #[diesel(sql_type = Binary)]
            value: Rational,
        }

        let res = sql_query("SELECT ? as value")
            .bind::<Binary, _>(Rational(Ratio::new(3u32.into(), 14u32.into())))
            .load::<Row>(&mut conn)?;

        assert_eq!(
            &[Row {
                value: Rational(Ratio::new(3u32.into(), 14u32.into()))
            }],
            res.as_slice()
        );

        Ok(())
    }

    #[test]
    fn db_invalid_value_gives_error() -> Result<(), Box<dyn Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;

        #[derive(QueryableByName, PartialEq, Eq, Debug)]
        struct Row {
            #[diesel(sql_type = Binary)]
            value: Rational,
        }

        let res = sql_query("SELECT X'' as value").load::<Row>(&mut conn);
        assert!(res.is_err());

        let res = sql_query("SELECT X'00000000' as value").load::<Row>(&mut conn);
        assert!(res.is_err());

        let res = sql_query("SELECT X'00000001' as value").load::<Row>(&mut conn);
        assert!(res.is_err());

        let res = sql_query("SELECT X'01000000' as value").load::<Row>(&mut conn);
        assert!(res.is_err());

        let res = sql_query("SELECT X'0100000001' as value").load::<Row>(&mut conn);
        assert!(res.is_err());

        // 1/1
        let res = sql_query("SELECT X'010000000101' as value").load::<Row>(&mut conn);
        assert!(res.is_ok());

        // 1/1 with trailing zeroes (i.e. leading in big endian, so not contributing any value)
        let res = sql_query("SELECT X'0100000001010000' as value").load::<Row>(&mut conn);
        assert!(res.is_ok());

        // 1/1 with trailing zeroes (i.e. leading in big endian, so not contributing any value)
        let res = sql_query("SELECT X'020000000100010000' as value").load::<Row>(&mut conn);
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn sum_rat() -> Result<(), Box<dyn Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        sum_rat::register_impl::<SumRat, _>(&mut conn).unwrap();

        #[derive(QueryableByName, PartialEq, Eq, Debug)]
        struct Row {
            #[diesel(sql_type = Binary)]
            value: Rational,
        }

        let res = sql_query("WITH t(x) AS (VALUES (?),(?)) SELECT sum_rat(x) as value FROM t")
            .bind::<Binary, _>(Rational::new(3u32, 14u32))
            .bind::<Binary, _>(Rational::new(2u32, 14u32))
            .load::<Row>(&mut conn)
            .unwrap();

        assert_eq!(
            &[Row {
                value: Rational::new(5u32, 14u32)
            }],
            res.as_slice()
        );

        Ok(())
    }
}
