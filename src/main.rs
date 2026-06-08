// build_rationale and the brand() data constructor legitimately take many
// positional args; the stats query returns a small internal tuple. Grouping
// these into structs would hurt readability more than help.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

mod barcode_seeds;
mod brand_data;
mod db;
mod error;
mod handlers;
mod middleware;
mod models;
mod state;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

use state::AppState;

const DB_PATH: &str = "rewoven.db";

/// The data routes, with version-relative paths. Mounted under both `/api`
/// (legacy, kept for existing clients) and `/v1` (the canonical, versioned base).
fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/brands", get(handlers::brands::list_brands))
        .route("/brands/search", get(handlers::brands::search_brands))
        .route("/brands/top", get(handlers::brands::top_brands))
        .route("/brands/worst", get(handlers::brands::worst_brands))
        .route("/brands/compare", get(handlers::brands::compare_brands))
        .route("/brands/{slug}", get(handlers::brands::get_brand))
        .route(
            "/brands/{slug}/alternatives",
            get(handlers::brands::get_alternatives),
        )
        .route("/materials", get(handlers::materials::get_materials))
        .route("/materials/{slug}", get(handlers::materials::get_material))
        .route("/categories", get(handlers::stats::get_categories))
        .route("/stats", get(handlers::stats::get_stats))
        .route(
            "/barcode/contribute",
            post(handlers::barcode::contribute_barcode),
        )
        .route("/barcode/{upc}", get(handlers::barcode::lookup_barcode))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Connection pool (replaces single Mutex<Connection>)
    let pool = db::create_pool(DB_PATH).expect("Failed to create connection pool");

    // Init and seed using one connection from the pool
    {
        let conn = pool.get().expect("Failed to get connection for init");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .ok();
        db::init_db(&conn).expect("Failed to initialize database");
        let brands = brand_data::load_brands();
        db::seed_db(&conn, &brands).expect("Failed to seed database");
        let count = db::get_brand_count(&conn).unwrap_or(0);
        tracing::info!("Database has {} brands", count);

        // Seed barcode prefixes (idempotent - only inserts new ones)
        match db::seed_barcode_prefixes(&conn, barcode_seeds::SEED_PREFIXES) {
            Ok(inserted) => {
                let total = db::get_barcode_prefix_count(&conn).unwrap_or(0);
                tracing::info!(
                    "Barcode prefixes: {} new, {} total ({} in seed file)",
                    inserted,
                    total,
                    barcode_seeds::count(),
                );
            }
            Err(e) => tracing::error!("Failed to seed barcode prefixes: {}", e),
        }
    }

    let state = Arc::new(AppState { db: pool });

    // Per-IP rate limiter (public API protection). Generous, so the extension
    // and site stay well under it; abusive scripts get 429'd.
    let limiter = middleware::new_limiter(120);
    {
        // Periodically drop idle per-IP buckets so memory stays bounded.
        let l = limiter.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(600));
            loop {
                tick.tick().await;
                l.retain_recent();
            }
        });
    }

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
        .route("/openapi.json", get(handlers::docs::openapi_json))
        .route("/docs", get(handlers::docs::swagger_ui))
        .nest("/api", api_routes()) // legacy base, kept working
        .nest("/v1", api_routes()) // canonical versioned base
        .with_state(state)
        // Caching (Cache-Control + ETag/304) then per-IP rate limiting, with
        // CORS outermost so preflight is handled first.
        .layer(axum::middleware::from_fn(middleware::cache_and_etag))
        .layer(axum::middleware::from_fn_with_state(
            limiter.clone(),
            middleware::rate_limit,
        ))
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Rewoven API starting on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");
    axum::serve(listener, app).await.expect("Server error");
}
