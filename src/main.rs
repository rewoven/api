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
use tower_http::cors::{Any, CorsLayer};

use state::AppState;

const DB_PATH: &str = "rewoven.db";

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
        .route(
            "/brands/{slug}/badge.svg",
            get(handlers::brands::brand_badge),
        )
        .route("/export.json", get(handlers::export::export_json))
        .route("/export.csv", get(handlers::export::export_csv))
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

    let pool = db::create_pool(DB_PATH).expect("Failed to create connection pool");

    {
        let conn = pool.get().expect("Failed to get connection for init");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .ok();
        db::init_db(&conn).expect("Failed to initialize database");
        db::run_migrations(&conn).expect("Failed to run migrations");
        let brands = brand_data::load_brands();
        db::seed_db(&conn, &brands).expect("Failed to seed database");
        let count = db::get_brand_count(&conn).unwrap_or(0);
        tracing::info!("Database has {} brands", count);

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

    let limiter = middleware::new_limiter(120);
    {
        let l = limiter.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(600));
            loop {
                tick.tick().await;
                l.retain_recent();
            }
        });
    }

    // Fully public data API: any origin may read it. Abuse is handled by the
    // per-IP rate limiter, not by CORS.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(handlers::brands::health))
        .route("/openapi.json", get(handlers::docs::openapi_json))
        .route("/docs", get(handlers::docs::swagger_ui))
        .nest("/api", api_routes())
        .nest("/v1", api_routes())
        .with_state(state)
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
