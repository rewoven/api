mod brand_data;
mod db;
mod error;
mod handlers;
mod models;
mod state;

use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

use state::AppState;

const DB_PATH: &str = "rewoven.db";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Connection pool (replaces single Mutex<Connection>)
    let pool = db::create_pool(DB_PATH).expect("Failed to create connection pool");

    // Init and seed using one connection from the pool
    {
        let conn = pool.get().expect("Failed to get connection for init");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;").ok();
        db::init_db(&conn).expect("Failed to initialize database");
        let brands = brand_data::load_brands();
        db::seed_db(&conn, &brands).expect("Failed to seed database");
        let count = db::get_brand_count(&conn).unwrap_or(0);
        tracing::info!("Database has {} brands", count);
    }

    let state = Arc::new(AppState { db: pool });

    // CORS locked to known origins
    let origins = [
        "https://rewovenapp.com",
        "https://www.rewovenapp.com",
        "http://localhost:3000",
        "http://localhost:5173",
        "http://localhost:5500",
        "http://127.0.0.1:5500",
    ];
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            origins.iter().filter_map(|o| o.parse().ok()),
        ))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/health", get(handlers::brands::health))
        .route("/api/brands", get(handlers::brands::list_brands))
        .route("/api/brands/search", get(handlers::brands::search_brands))
        .route("/api/brands/top", get(handlers::brands::top_brands))
        .route("/api/brands/worst", get(handlers::brands::worst_brands))
        .route("/api/brands/compare", get(handlers::brands::compare_brands))
        .route("/api/brands/{slug}", get(handlers::brands::get_brand))
        .route("/api/brands/{slug}/alternatives", get(handlers::brands::get_alternatives))
        .route("/api/materials", get(handlers::materials::get_materials))
        .route("/api/materials/{slug}", get(handlers::materials::get_material))
        .route("/api/categories", get(handlers::stats::get_categories))
        .route("/api/stats", get(handlers::stats::get_stats))
        .with_state(state)
        .layer(cors);

    let addr = "0.0.0.0:3000";
    tracing::info!("Rewoven API starting on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");
    axum::serve(listener, app)
        .await
        .expect("Server error");
}
