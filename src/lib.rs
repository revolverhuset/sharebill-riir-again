use diesel::prelude::*;
use diesel::SqliteConnection;

pub mod models;
pub mod schema;

pub fn establish_connection() -> SqliteConnection {
    let database_url = "test.db";

    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
