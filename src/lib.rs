use diesel::{prelude::*, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rational::{sum_rat, SumRat};

pub mod models;
pub mod rational;
pub mod schema;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn establish_connection(database_url: &str) -> SqliteConnection {
    let mut con = SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    con.run_pending_migrations(MIGRATIONS).unwrap();

    sum_rat::register_impl::<SumRat, _>(&mut con).unwrap();

    con
}
