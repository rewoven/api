mod brands;

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber;

use brands::BrandRating;

const DB_PATH: &str = "rewoven.db";

struct AppState {
    db: Mutex<Connection>,
}

// Query parameters
#[derive(Deserialize)]
struct ListParams {
    page: Option<usize>,
    limit: Option<usize>,
    category: Option<String>,
    min_score: Option<u8>,
    max_score: Option<u8>,
    search: Option<String>,
    sort: Option<String>,
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
}

#[derive(Deserialize)]
struct LimitParams {
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct CompareParams {
    brands: Option<String>,
}

#[derive(Deserialize)]
struct AlternativesParams {
    limit: Option<usize>,
    min_score: Option<u8>,
}

// Material impact data
#[derive(Serialize, Clone)]
struct MaterialImpact {
    name: String,
    slug: String,
    category: String,
    co2_kg_per_kg: f64,
    water_liters_per_kg: f64,
    biodegradable: bool,
    recyclable: bool,
    sustainability_score: u8,
    description: String,
}

// Response types
#[derive(Serialize)]
struct AlternativesResponse {
    original: BrandRating,
    alternatives: Vec<BrandRating>,
    reason: String,
}

#[derive(Serialize)]
struct PaginatedResponse {
    brands: Vec<BrandRating>,
    total: usize,
    page: usize,
    pages: usize,
}

#[derive(Serialize)]
struct CategoryStats {
    category: String,
    count: usize,
    average_score: f64,
    average_environmental: f64,
    average_labor: f64,
    average_transparency: f64,
    average_animal_welfare: f64,
}

#[derive(Serialize)]
struct OverallStats {
    total_brands: usize,
    average_score: f64,
    median_score: u8,
    grade_distribution: std::collections::HashMap<String, usize>,
    category_count: usize,
    categories: Vec<CategoryStats>,
    price_range_distribution: std::collections::HashMap<String, usize>,
    country_count: usize,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    status: u16,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    total_brands: usize,
}

// ─── Database ───

fn init_db(conn: &Connection) {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS brands (
            slug TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            overall_score INTEGER NOT NULL,
            grade TEXT NOT NULL,
            environmental_score INTEGER NOT NULL,
            labor_score INTEGER NOT NULL,
            transparency_score INTEGER NOT NULL,
            animal_welfare_score INTEGER NOT NULL,
            price_range TEXT NOT NULL,
            country TEXT NOT NULL,
            category TEXT NOT NULL,
            certifications TEXT NOT NULL DEFAULT '[]',
            summary TEXT NOT NULL DEFAULT '',
            website TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
    ").expect("Failed to create brands table");
}

fn seed_db(conn: &Connection, brands: Vec<BrandRating>) {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM brands", [], |row| row.get(0))
        .unwrap_or(0);

    if count > 0 {
        tracing::info!("Database already has {} brands, skipping seed", count);
        return;
    }

    tracing::info!("Seeding database with {} brands...", brands.len());
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO brands (slug, name, overall_score, grade, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
    ).unwrap();

    for b in &brands {
        let certs_json = serde_json::to_string(&b.certifications).unwrap_or_else(|_| "[]".to_string());
        stmt.execute(params![
            b.slug, b.name, b.overall_score, b.grade,
            b.environmental_score, b.labor_score, b.transparency_score, b.animal_welfare_score,
            b.price_range, b.country, b.category, certs_json, b.summary, b.website
        ]).ok();
    }
    tracing::info!("Seeded {} brands into database", brands.len());
}

fn db_get_all_brands(conn: &Connection) -> Vec<BrandRating> {
    let mut stmt = conn.prepare(
        "SELECT slug, name, overall_score, grade, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website FROM brands"
    ).unwrap();

    stmt.query_map([], |row| {
        let certs_str: String = row.get(11)?;
        let certifications: Vec<String> = serde_json::from_str(&certs_str).unwrap_or_default();
        Ok(BrandRating {
            slug: row.get(0)?,
            name: row.get(1)?,
            overall_score: row.get(2)?,
            grade: row.get(3)?,
            environmental_score: row.get(4)?,
            labor_score: row.get(5)?,
            transparency_score: row.get(6)?,
            animal_welfare_score: row.get(7)?,
            price_range: row.get(8)?,
            country: row.get(9)?,
            category: row.get(10)?,
            certifications,
            summary: row.get(12)?,
            website: row.get(13)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect()
}

fn db_upsert_brand(conn: &Connection, b: &BrandRating) -> bool {
    let certs_json = serde_json::to_string(&b.certifications).unwrap_or_else(|_| "[]".to_string());
    let exists: bool = conn
        .query_row("SELECT COUNT(*) FROM brands WHERE slug = ?1", params![b.slug], |row| row.get::<_, i64>(0))
        .map(|c| c > 0)
        .unwrap_or(false);

    conn.execute(
        "INSERT INTO brands (slug, name, overall_score, grade, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, datetime('now'))
         ON CONFLICT(slug) DO UPDATE SET
            name=excluded.name, overall_score=excluded.overall_score, grade=excluded.grade,
            environmental_score=excluded.environmental_score, labor_score=excluded.labor_score,
            transparency_score=excluded.transparency_score, animal_welfare_score=excluded.animal_welfare_score,
            price_range=excluded.price_range, country=excluded.country, category=excluded.category,
            certifications=excluded.certifications, summary=excluded.summary, website=excluded.website,
            updated_at=datetime('now')",
        params![
            b.slug, b.name, b.overall_score, b.grade,
            b.environmental_score, b.labor_score, b.transparency_score, b.animal_welfare_score,
            b.price_range, b.country, b.category, certs_json, b.summary, b.website
        ],
    ).ok();

    exists // true = updated, false = added
}

// ─── Main ───

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Open SQLite database
    let conn = Connection::open(DB_PATH).expect("Failed to open database");
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;").ok();
    init_db(&conn);

    // Seed from hardcoded data if empty
    let hardcoded_brands = brands::load_brands();
    seed_db(&conn, hardcoded_brands);

    let brand_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM brands", [], |row| row.get(0))
        .unwrap_or(0);
    tracing::info!("Database has {} brands", brand_count);

    let state = Arc::new(AppState {
        db: Mutex::new(conn),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get({
            let state = Arc::clone(&state);
            move || health(state)
        }))
        .route("/api/brands", get({
            let state = Arc::clone(&state);
            move |query| list_brands(query, state)
        }))
        .route("/api/brands/search", get({
            let state = Arc::clone(&state);
            move |query| search_brands(query, state)
        }))
        .route("/api/brands/top", get({
            let state = Arc::clone(&state);
            move |query| top_brands(query, state)
        }))
        .route("/api/brands/worst", get({
            let state = Arc::clone(&state);
            move |query| worst_brands(query, state)
        }))
        .route("/api/brands/compare", get({
            let state = Arc::clone(&state);
            move |query| compare_brands(query, state)
        }))
        .route("/api/brands/{slug}", get({
            let state = Arc::clone(&state);
            move |path| get_brand(path, state)
        }))
        .route("/api/brands/{slug}/alternatives", get({
            let state = Arc::clone(&state);
            move |path, query| get_alternatives(path, query, state)
        }))
        .route("/api/materials", get(get_materials))
        .route("/api/materials/{slug}", get(get_material))
        .route("/api/categories", get({
            let state = Arc::clone(&state);
            move || get_categories(state)
        }))
.route("/api/stats", get({
            let state = Arc::clone(&state);
            move || get_stats(state)
        }))
        .layer(cors);

    let addr = "0.0.0.0:3000";
    tracing::info!("Rewoven API starting on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ─── Handlers ───

async fn health(state: Arc<AppState>) -> Json<HealthResponse> {
    let db = state.db.lock().await;
    let count: usize = db
        .query_row("SELECT COUNT(*) FROM brands", [], |row| row.get(0))
        .unwrap_or(0);
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        total_brands: count,
    })
}

async fn list_brands(
    Query(params): Query<ListParams>,
    state: Arc<AppState>,
) -> Json<PaginatedResponse> {
    let db = state.db.lock().await;
    let mut all_brands = db_get_all_brands(&db);

    // Filter by category
    if let Some(ref category) = params.category {
        let cat_lower = category.to_lowercase();
        all_brands.retain(|b| b.category.to_lowercase() == cat_lower);
    }

    if let Some(min) = params.min_score {
        all_brands.retain(|b| b.overall_score >= min);
    }
    if let Some(max) = params.max_score {
        all_brands.retain(|b| b.overall_score <= max);
    }

    if let Some(ref search) = params.search {
        let search_lower = search.to_lowercase();
        all_brands.retain(|b| b.name.to_lowercase().contains(&search_lower));
    }

    match params.sort.as_deref() {
        Some("score_desc") => all_brands.sort_by(|a, b| b.overall_score.cmp(&a.overall_score)),
        Some("score_asc") => all_brands.sort_by(|a, b| a.overall_score.cmp(&b.overall_score)),
        Some("name_asc") => all_brands.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        Some("name_desc") => all_brands.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase())),
        _ => {}
    }

    let total = all_brands.len();
    let limit = params.limit.unwrap_or(50).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let pages = if total == 0 { 1 } else { (total + limit - 1) / limit };
    let start = (page - 1) * limit;

    let brands: Vec<BrandRating> = all_brands.into_iter().skip(start).take(limit).collect();

    Json(PaginatedResponse { brands, total, page, pages })
}

async fn get_brand(
    Path(slug): Path<String>,
    state: Arc<AppState>,
) -> Result<Json<BrandRating>, impl IntoResponse> {
    let slug_lower = slug.to_lowercase();
    let db = state.db.lock().await;
    let brands = db_get_all_brands(&db);
    match brands.into_iter().find(|b| b.slug == slug_lower) {
        Some(brand) => Ok(Json(brand)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: format!("Brand '{}' not found", slug), status: 404 }),
        )),
    }
}

async fn search_brands(
    Query(params): Query<SearchParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let query = match params.q {
        Some(q) if !q.is_empty() => q.to_lowercase(),
        _ => return Json(vec![]),
    };

    let db = state.db.lock().await;
    let brands = db_get_all_brands(&db);
    let mut results: Vec<(usize, BrandRating)> = brands
        .into_iter()
        .filter_map(|b| {
            let name_lower = b.name.to_lowercase();
            if name_lower == query {
                Some((0, b))
            } else if name_lower.starts_with(&query) {
                Some((1, b))
            } else if name_lower.contains(&query) {
                Some((2, b))
            } else if fuzzy_match(&name_lower, &query) {
                Some((3, b))
            } else {
                None
            }
        })
        .collect();

    results.sort_by_key(|(priority, _)| *priority);
    Json(results.into_iter().map(|(_, b)| b).collect())
}

fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    if needle.len() < 3 {
        return false;
    }
    let mut hay_chars = haystack.chars().peekable();
    let mut matched = 0;
    for nc in needle.chars() {
        while let Some(&hc) = hay_chars.peek() {
            hay_chars.next();
            if hc == nc {
                matched += 1;
                break;
            }
        }
    }
    let threshold = if needle.len() <= 4 { needle.len() - 1 } else { needle.len() - 2 };
    matched >= threshold
}

async fn top_brands(
    Query(params): Query<LimitParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let limit = params.limit.unwrap_or(10).min(50);
    let db = state.db.lock().await;
    let mut brands = db_get_all_brands(&db);
    brands.sort_by(|a, b| b.overall_score.cmp(&a.overall_score));
    brands.truncate(limit);
    Json(brands)
}

async fn worst_brands(
    Query(params): Query<LimitParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let limit = params.limit.unwrap_or(10).min(50);
    let db = state.db.lock().await;
    let mut brands = db_get_all_brands(&db);
    brands.sort_by(|a, b| a.overall_score.cmp(&b.overall_score));
    brands.truncate(limit);
    Json(brands)
}

async fn compare_brands(
    Query(params): Query<CompareParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let slugs = match params.brands {
        Some(ref b) => b.split(',').map(|s| s.trim().to_lowercase()).collect::<Vec<_>>(),
        None => return Json(vec![]),
    };

    let db = state.db.lock().await;
    let brands = db_get_all_brands(&db);
    let results: Vec<BrandRating> = slugs
        .iter()
        .filter_map(|slug| brands.iter().find(|b| b.slug == *slug).cloned())
        .collect();

    Json(results)
}

async fn get_categories(state: Arc<AppState>) -> Json<Vec<CategoryStats>> {
    let db = state.db.lock().await;
    let brands = db_get_all_brands(&db);
    let mut category_map: std::collections::HashMap<String, Vec<&BrandRating>> =
        std::collections::HashMap::new();

    for brand in &brands {
        category_map.entry(brand.category.clone()).or_default().push(brand);
    }

    let mut categories: Vec<CategoryStats> = category_map
        .into_iter()
        .map(|(category, cat_brands)| {
            let count = cat_brands.len();
            let avg = |f: fn(&BrandRating) -> u8| -> f64 {
                cat_brands.iter().map(|b| f(b) as f64).sum::<f64>() / count as f64
            };
            CategoryStats {
                category,
                count,
                average_score: (avg(|b| b.overall_score) * 10.0).round() / 10.0,
                average_environmental: (avg(|b| b.environmental_score) * 10.0).round() / 10.0,
                average_labor: (avg(|b| b.labor_score) * 10.0).round() / 10.0,
                average_transparency: (avg(|b| b.transparency_score) * 10.0).round() / 10.0,
                average_animal_welfare: (avg(|b| b.animal_welfare_score) * 10.0).round() / 10.0,
            }
        })
        .collect();

    categories.sort_by(|a, b| b.average_score.partial_cmp(&a.average_score).unwrap());
    Json(categories)
}

async fn get_stats(state: Arc<AppState>) -> Json<OverallStats> {
    let db = state.db.lock().await;
    let brands = db_get_all_brands(&db);
    let total = brands.len();

    if total == 0 {
        return Json(OverallStats {
            total_brands: 0, average_score: 0.0, median_score: 0,
            grade_distribution: Default::default(), category_count: 0,
            categories: vec![], price_range_distribution: Default::default(), country_count: 0,
        });
    }

    let avg_score = brands.iter().map(|b| b.overall_score as f64).sum::<f64>() / total as f64;

    let mut scores: Vec<u8> = brands.iter().map(|b| b.overall_score).collect();
    scores.sort();
    let median = scores[total / 2];

    let mut grade_dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for brand in &brands {
        *grade_dist.entry(brand.grade.clone()).or_insert(0) += 1;
    }

    let mut price_dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for brand in &brands {
        *price_dist.entry(brand.price_range.clone()).or_insert(0) += 1;
    }

    let countries: std::collections::HashSet<&str> = brands.iter().map(|b| b.country.as_str()).collect();

    let mut category_map: std::collections::HashMap<String, Vec<&BrandRating>> =
        std::collections::HashMap::new();
    for brand in &brands {
        category_map.entry(brand.category.clone()).or_default().push(brand);
    }

    let mut categories: Vec<CategoryStats> = category_map
        .into_iter()
        .map(|(category, cat_brands)| {
            let count = cat_brands.len();
            let avg = |f: fn(&BrandRating) -> u8| -> f64 {
                cat_brands.iter().map(|b| f(b) as f64).sum::<f64>() / count as f64
            };
            CategoryStats {
                category,
                count,
                average_score: (avg(|b| b.overall_score) * 10.0).round() / 10.0,
                average_environmental: (avg(|b| b.environmental_score) * 10.0).round() / 10.0,
                average_labor: (avg(|b| b.labor_score) * 10.0).round() / 10.0,
                average_transparency: (avg(|b| b.transparency_score) * 10.0).round() / 10.0,
                average_animal_welfare: (avg(|b| b.animal_welfare_score) * 10.0).round() / 10.0,
            }
        })
        .collect();

    categories.sort_by(|a, b| b.average_score.partial_cmp(&a.average_score).unwrap());

    Json(OverallStats {
        total_brands: total,
        average_score: (avg_score * 10.0).round() / 10.0,
        median_score: median,
        grade_distribution: grade_dist,
        category_count: categories.len(),
        categories,
        price_range_distribution: price_dist,
        country_count: countries.len(),
    })
}

async fn get_alternatives(
    Path(slug): Path<String>,
    Query(params): Query<AlternativesParams>,
    state: Arc<AppState>,
) -> Result<Json<AlternativesResponse>, impl IntoResponse> {
    let slug_lower = slug.to_lowercase();
    let db = state.db.lock().await;
    let brands = db_get_all_brands(&db);

    let brand = match brands.iter().find(|b| b.slug == slug_lower) {
        Some(b) => b.clone(),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse { error: format!("Brand '{}' not found", slug), status: 404 }),
            ))
        }
    };

    let limit = params.limit.unwrap_or(5).min(20);
    let min_score = params.min_score.unwrap_or(brand.overall_score.saturating_add(10));

    let price_tiers: Vec<&str> = match brand.price_range.as_str() {
        "$" => vec!["$", "$$"],
        "$$" => vec!["$", "$$", "$$$"],
        "$$$" => vec!["$$", "$$$", "$$$$"],
        "$$$$" => vec!["$$$", "$$$$"],
        _ => vec!["$", "$$", "$$$", "$$$$"],
    };

    let mut alternatives: Vec<(u32, &BrandRating)> = brands
        .iter()
        .filter(|b| b.slug != slug_lower && b.overall_score >= min_score)
        .map(|b| {
            let mut relevance: u32 = 0;
            if b.category.to_lowercase() == brand.category.to_lowercase() {
                relevance += 100;
            }
            if price_tiers.contains(&b.price_range.as_str()) {
                relevance += 50;
            }
            relevance += b.overall_score as u32;
            (relevance, b)
        })
        .collect();

    alternatives.sort_by(|a, b| b.0.cmp(&a.0));

    let alts: Vec<BrandRating> = alternatives.into_iter().take(limit).map(|(_, b)| b.clone()).collect();

    let reason = format!(
        "Showing sustainable alternatives to {} (score: {}/100, grade: {}). These brands score higher on sustainability while offering similar style and price range.",
        brand.name, brand.overall_score, brand.grade
    );

    Ok(Json(AlternativesResponse { original: brand, alternatives: alts, reason }))
}

// ─── Materials (still hardcoded — small static dataset) ───

fn load_materials() -> Vec<MaterialImpact> {
    vec![
        MaterialImpact { name: "Conventional Cotton".into(), slug: "conventional-cotton".into(), category: "Natural".into(), co2_kg_per_kg: 8.0, water_liters_per_kg: 10000.0, biodegradable: true, recyclable: true, sustainability_score: 35, description: "Most widely used natural fiber. Extremely water-intensive and often relies on pesticides.".into() },
        MaterialImpact { name: "Organic Cotton".into(), slug: "organic-cotton".into(), category: "Natural".into(), co2_kg_per_kg: 4.0, water_liters_per_kg: 7000.0, biodegradable: true, recyclable: true, sustainability_score: 72, description: "Grown without synthetic pesticides or fertilizers. Uses less water than conventional cotton.".into() },
        MaterialImpact { name: "Polyester".into(), slug: "polyester".into(), category: "Synthetic".into(), co2_kg_per_kg: 9.5, water_liters_per_kg: 60.0, biodegradable: false, recyclable: true, sustainability_score: 20, description: "Derived from petroleum. Low water use but high carbon footprint and sheds microplastics.".into() },
        MaterialImpact { name: "Recycled Polyester".into(), slug: "recycled-polyester".into(), category: "Recycled".into(), co2_kg_per_kg: 3.5, water_liters_per_kg: 40.0, biodegradable: false, recyclable: true, sustainability_score: 58, description: "Made from recycled PET bottles. 59% less energy than virgin polyester but still sheds microplastics.".into() },
        MaterialImpact { name: "Nylon".into(), slug: "nylon".into(), category: "Synthetic".into(), co2_kg_per_kg: 12.0, water_liters_per_kg: 100.0, biodegradable: false, recyclable: true, sustainability_score: 15, description: "Petroleum-based with very high CO2 emissions. Produces nitrous oxide, a potent greenhouse gas.".into() },
        MaterialImpact { name: "Recycled Nylon".into(), slug: "recycled-nylon".into(), category: "Recycled".into(), co2_kg_per_kg: 5.0, water_liters_per_kg: 60.0, biodegradable: false, recyclable: true, sustainability_score: 55, description: "Made from ocean waste and old fishing nets. Significantly lower impact than virgin nylon.".into() },
        MaterialImpact { name: "Linen".into(), slug: "linen".into(), category: "Natural".into(), co2_kg_per_kg: 1.5, water_liters_per_kg: 700.0, biodegradable: true, recyclable: true, sustainability_score: 85, description: "Made from flax plant. Very low water and pesticide needs. One of the most sustainable fabrics.".into() },
        MaterialImpact { name: "Hemp".into(), slug: "hemp".into(), category: "Natural".into(), co2_kg_per_kg: 1.2, water_liters_per_kg: 500.0, biodegradable: true, recyclable: true, sustainability_score: 90, description: "Requires minimal water, no pesticides, and improves soil health. Extremely sustainable choice.".into() },
        MaterialImpact { name: "Wool".into(), slug: "wool".into(), category: "Animal".into(), co2_kg_per_kg: 17.0, water_liters_per_kg: 15000.0, biodegradable: true, recyclable: true, sustainability_score: 40, description: "Natural and biodegradable but high water use and methane emissions from sheep farming.".into() },
        MaterialImpact { name: "Merino Wool".into(), slug: "merino-wool".into(), category: "Animal".into(), co2_kg_per_kg: 20.0, water_liters_per_kg: 17000.0, biodegradable: true, recyclable: true, sustainability_score: 38, description: "Premium wool with mulesing concerns. Durable and naturally temperature-regulating.".into() },
        MaterialImpact { name: "Silk".into(), slug: "silk".into(), category: "Animal".into(), co2_kg_per_kg: 15.0, water_liters_per_kg: 10000.0, biodegradable: true, recyclable: false, sustainability_score: 30, description: "Natural luxury fiber but involves killing silkworms. High water and energy consumption.".into() },
        MaterialImpact { name: "Peace Silk".into(), slug: "peace-silk".into(), category: "Animal".into(), co2_kg_per_kg: 16.0, water_liters_per_kg: 10500.0, biodegradable: true, recyclable: false, sustainability_score: 45, description: "Cruelty-free silk that allows moths to emerge before harvesting. Higher ethical standards.".into() },
        MaterialImpact { name: "Viscose/Rayon".into(), slug: "viscose-rayon".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 7.0, water_liters_per_kg: 3000.0, biodegradable: true, recyclable: false, sustainability_score: 30, description: "Made from wood pulp using chemical-intensive process. Often linked to deforestation.".into() },
        MaterialImpact { name: "Tencel/Lyocell".into(), slug: "tencel-lyocell".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 2.0, water_liters_per_kg: 1500.0, biodegradable: true, recyclable: true, sustainability_score: 82, description: "Made from sustainably sourced wood pulp in a closed-loop process. Very eco-friendly.".into() },
        MaterialImpact { name: "Modal".into(), slug: "modal".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 3.0, water_liters_per_kg: 2000.0, biodegradable: true, recyclable: true, sustainability_score: 70, description: "Made from beech tree pulp. More sustainable than viscose when sourced from managed forests.".into() },
        MaterialImpact { name: "Bamboo".into(), slug: "bamboo".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 4.0, water_liters_per_kg: 800.0, biodegradable: true, recyclable: false, sustainability_score: 50, description: "Bamboo grows fast without pesticides but processing into fabric uses harsh chemicals.".into() },
        MaterialImpact { name: "Acrylic".into(), slug: "acrylic".into(), category: "Synthetic".into(), co2_kg_per_kg: 11.5, water_liters_per_kg: 200.0, biodegradable: false, recyclable: false, sustainability_score: 10, description: "Petroleum-based with high CO2 and toxic chemical use. Not recyclable or biodegradable.".into() },
        MaterialImpact { name: "Spandex/Elastane".into(), slug: "spandex-elastane".into(), category: "Synthetic".into(), co2_kg_per_kg: 10.0, water_liters_per_kg: 150.0, biodegradable: false, recyclable: false, sustainability_score: 12, description: "Petroleum-based stretch fiber. Cannot be recycled and makes blended fabrics harder to recycle.".into() },
        MaterialImpact { name: "Leather".into(), slug: "leather".into(), category: "Animal".into(), co2_kg_per_kg: 25.0, water_liters_per_kg: 17000.0, biodegradable: true, recyclable: false, sustainability_score: 18, description: "Extremely high environmental impact from cattle farming, tanning chemicals, and water use.".into() },
        MaterialImpact { name: "Vegan Leather (PU)".into(), slug: "vegan-leather-pu".into(), category: "Synthetic".into(), co2_kg_per_kg: 8.0, water_liters_per_kg: 200.0, biodegradable: false, recyclable: false, sustainability_score: 28, description: "Polyurethane-based alternative. Lower impact than leather but still petroleum-derived.".into() },
        MaterialImpact { name: "Piñatex".into(), slug: "pinatex".into(), category: "Innovative".into(), co2_kg_per_kg: 2.5, water_liters_per_kg: 300.0, biodegradable: true, recyclable: false, sustainability_score: 78, description: "Made from pineapple leaf fibers. Innovative, natural, and uses agricultural waste.".into() },
        MaterialImpact { name: "Mushroom Leather (Mylo)".into(), slug: "mushroom-leather".into(), category: "Innovative".into(), co2_kg_per_kg: 1.8, water_liters_per_kg: 200.0, biodegradable: true, recyclable: false, sustainability_score: 85, description: "Grown from mycelium in days. Very low environmental impact and fully biodegradable.".into() },
        MaterialImpact { name: "Recycled Cotton".into(), slug: "recycled-cotton".into(), category: "Recycled".into(), co2_kg_per_kg: 2.5, water_liters_per_kg: 1500.0, biodegradable: true, recyclable: true, sustainability_score: 75, description: "Made from pre- and post-consumer cotton waste. Significantly reduces water and energy use.".into() },
        MaterialImpact { name: "Cashmere".into(), slug: "cashmere".into(), category: "Animal".into(), co2_kg_per_kg: 28.0, water_liters_per_kg: 20000.0, biodegradable: true, recyclable: true, sustainability_score: 15, description: "Luxury fiber with severe environmental impact from goat overgrazing and desertification.".into() },
        MaterialImpact { name: "Down".into(), slug: "down".into(), category: "Animal".into(), co2_kg_per_kg: 22.0, water_liters_per_kg: 14000.0, biodegradable: true, recyclable: false, sustainability_score: 25, description: "Excellent insulator but serious animal welfare concerns with live-plucking and force-feeding.".into() },
        MaterialImpact { name: "Recycled Down".into(), slug: "recycled-down".into(), category: "Recycled".into(), co2_kg_per_kg: 3.0, water_liters_per_kg: 500.0, biodegradable: true, recyclable: false, sustainability_score: 70, description: "Reclaimed from old products. Same performance with dramatically lower environmental impact.".into() },
        MaterialImpact { name: "Econyl".into(), slug: "econyl".into(), category: "Recycled".into(), co2_kg_per_kg: 4.5, water_liters_per_kg: 50.0, biodegradable: false, recyclable: true, sustainability_score: 65, description: "Regenerated nylon from ocean waste, fabric scraps, and old carpets. Infinitely recyclable.".into() },
        MaterialImpact { name: "Cork Fabric".into(), slug: "cork-fabric".into(), category: "Innovative".into(), co2_kg_per_kg: 0.8, water_liters_per_kg: 100.0, biodegradable: true, recyclable: true, sustainability_score: 92, description: "Harvested from cork oak bark without killing the tree. Carbon-negative and biodegradable.".into() },
        MaterialImpact { name: "Seacell".into(), slug: "seacell".into(), category: "Innovative".into(), co2_kg_per_kg: 1.5, water_liters_per_kg: 300.0, biodegradable: true, recyclable: false, sustainability_score: 80, description: "Made from seaweed and wood cellulose. Naturally antibacterial with minimal processing.".into() },
        MaterialImpact { name: "Orange Fiber".into(), slug: "orange-fiber".into(), category: "Innovative".into(), co2_kg_per_kg: 2.0, water_liters_per_kg: 250.0, biodegradable: true, recyclable: false, sustainability_score: 82, description: "Made from citrus juice byproducts. Turns waste into luxury silk-like fabric.".into() },
    ]
}

async fn get_materials() -> Json<Vec<MaterialImpact>> {
    let mut materials = load_materials();
    materials.sort_by(|a, b| b.sustainability_score.cmp(&a.sustainability_score));
    Json(materials)
}

async fn get_material(
    Path(slug): Path<String>,
) -> Result<Json<MaterialImpact>, impl IntoResponse> {
    let slug_lower = slug.to_lowercase();
    let materials = load_materials();
    match materials.into_iter().find(|m| m.slug == slug_lower) {
        Some(material) => Ok(Json(material)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: format!("Material '{}' not found", slug), status: 404 }),
        )),
    }
}
