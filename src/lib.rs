use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool};
use diesel::sql_types::*;
use diesel::{prelude::*, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use id30::Id30;
use rational::{sum_rat, SumRat};

pub mod models;
pub mod parse_arg; // for doctests
pub mod rational;
pub mod schema;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn establish_connection(database_url: &str) -> SqliteConnection {
    let mut con = SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    SqliteInitializer.on_acquire(&mut con).unwrap();
    con.run_pending_migrations(MIGRATIONS).unwrap();

    con
}

#[derive(Debug)]
struct SqliteInitializer;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqliteInitializer {
    fn on_acquire(&self, con: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        diesel::dsl::sql::<(Integer,)>("PRAGMA foreign_keys = ON")
            .execute(con)
            .map_err(|x| diesel::r2d2::Error::QueryError(x))?;

        sum_rat::register_impl::<SumRat, _>(con).unwrap();

        Ok(())
    }
}

pub fn create_pool<S: Into<String>>(
    connection_string: S,
) -> Result<Pool<ConnectionManager<SqliteConnection>>, Box<dyn std::error::Error>> {
    let manager = ConnectionManager::<SqliteConnection>::new(connection_string);
    let pool = Pool::builder()
        .connection_customizer(Box::new(SqliteInitializer {}))
        .build(manager)?;

    pool.get()?.run_pending_migrations(MIGRATIONS).unwrap();

    Ok(pool)
}

pub fn new_random_tx_id(conn: &mut SqliteConnection) -> Result<Id30, diesel::result::Error> {
    use rand::{distributions::Standard, prelude::*};

    for candidate in Standard.sample_iter(rand::thread_rng()) {
        if schema::txs::table
            .filter(schema::txs::id.eq(candidate))
            .count()
            .first(conn)
            .map(|x: i64| x)?
            == 0
        {
            return Ok(candidate);
        }
    }

    unreachable!()
}
