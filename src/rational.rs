use diesel::deserialize::{self, FromSql};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Binary;
use diesel::sqlite::{Sqlite, SqliteValue};
use diesel::{AsExpression, FromSqlRow};
use num::rational::Ratio;
use num::BigUint;

#[derive(PartialEq, Eq, Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = Binary)]
pub struct Rational(Ratio<BigUint>);

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

impl FromSql<Binary, Sqlite> for Rational {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let bytes = <*const [u8] as FromSql<Binary, Sqlite>>::from_sql(bytes)?;
        let bytes: &[u8] = unsafe { &*bytes };

        let numer_len = u32::from_le_bytes(bytes[0..4].try_into().unwrap());

        let (numer, denom) = bytes[4..].split_at(numer_len as usize);

        Ok(Rational(Ratio::new(
            BigUint::from_bytes_le(numer),
            BigUint::from_bytes_le(denom),
        )))
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
}
