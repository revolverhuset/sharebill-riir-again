use diesel::{prelude::*, sql_types::Binary, sqlite::SqliteAggregateFunction, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rational::Rational;

pub mod models;
pub mod rational;
pub mod schema;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

sql_function! {
    #[aggregate]
    fn sum_rat(x: Binary) -> Binary;
}

#[derive(Default)]
struct SumRat {
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

pub fn establish_connection(database_url: &str) -> SqliteConnection {
    let mut con = SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    con.run_pending_migrations(MIGRATIONS).unwrap();

    sum_rat::register_impl::<SumRat, _>(&mut con).unwrap();

    con
}

#[cfg(test)]
mod test {
    use super::*;

    use diesel::sql_query;

    fn test_connection() -> SqliteConnection {
        establish_connection(":memory:")
    }

    #[test]
    fn sum_rat() {
        let mut conn = test_connection();

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
    }
}
