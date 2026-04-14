use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub struct AppState {
    pub db: Pool<SqliteConnectionManager>,
}
