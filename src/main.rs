mod brands;

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber;

use brands::BrandRating;

// Shared application state
struct AppState {
    brands: Vec<BrandRating>,
}

// Query parameters for listing brands
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

// Query parameters for search
#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
}

// Query parameters for top/worst
#[derive(Deserialize)]
struct LimitParams {
    limit: Option<usize>,
}

// Query parameters for compare
#[derive(Deserialize)]
struct CompareParams {
    brands: Option<String>,
}

// Response types
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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let brands = brands::load_brands();
    tracing::info!("Loaded {} brands", brands.len());

    let state = Arc::new(AppState { brands });

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

// GET /health
async fn health(state: Arc<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        total_brands: state.brands.len(),
    })
}

// GET /api/brands
async fn list_brands(
    Query(params): Query<ListParams>,
    state: Arc<AppState>,
) -> Json<PaginatedResponse> {
    let mut filtered: Vec<&BrandRating> = state.brands.iter().collect();

    // Filter by category
    if let Some(ref category) = params.category {
        let cat_lower = category.to_lowercase();
        filtered.retain(|b| b.category.to_lowercase() == cat_lower);
    }

    // Filter by score range
    if let Some(min) = params.min_score {
        filtered.retain(|b| b.overall_score >= min);
    }
    if let Some(max) = params.max_score {
        filtered.retain(|b| b.overall_score <= max);
    }

    // Search filter
    if let Some(ref search) = params.search {
        let search_lower = search.to_lowercase();
        filtered.retain(|b| b.name.to_lowercase().contains(&search_lower));
    }

    // Sort
    match params.sort.as_deref() {
        Some("score_desc") => filtered.sort_by(|a, b| b.overall_score.cmp(&a.overall_score)),
        Some("score_asc") => filtered.sort_by(|a, b| a.overall_score.cmp(&b.overall_score)),
        Some("name_asc") => filtered.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        Some("name_desc") => filtered.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase())),
        _ => {} // default order
    }

    let total = filtered.len();
    let limit = params.limit.unwrap_or(50).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let pages = if total == 0 { 1 } else { (total + limit - 1) / limit };
    let start = (page - 1) * limit;

    let brands: Vec<BrandRating> = filtered
        .into_iter()
        .skip(start)
        .take(limit)
        .cloned()
        .collect();

    Json(PaginatedResponse {
        brands,
        total,
        page,
        pages,
    })
}

// GET /api/brands/:slug
async fn get_brand(
    Path(slug): Path<String>,
    state: Arc<AppState>,
) -> Result<Json<BrandRating>, impl IntoResponse> {
    let slug_lower = slug.to_lowercase();
    match state.brands.iter().find(|b| b.slug == slug_lower) {
        Some(brand) => Ok(Json(brand.clone())),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Brand '{}' not found", slug),
                status: 404,
            }),
        )),
    }
}

// GET /api/brands/search?q=zara
async fn search_brands(
    Query(params): Query<SearchParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let query = match params.q {
        Some(q) if !q.is_empty() => q.to_lowercase(),
        _ => return Json(vec![]),
    };

    let mut results: Vec<(usize, &BrandRating)> = state
        .brands
        .iter()
        .filter_map(|b| {
            let name_lower = b.name.to_lowercase();
            if name_lower == query {
                Some((0, b)) // exact match
            } else if name_lower.starts_with(&query) {
                Some((1, b)) // starts with
            } else if name_lower.contains(&query) {
                Some((2, b)) // contains
            } else if fuzzy_match(&name_lower, &query) {
                Some((3, b)) // fuzzy
            } else {
                None
            }
        })
        .collect();

    results.sort_by_key(|(priority, _)| *priority);
    Json(results.into_iter().map(|(_, b)| b.clone()).collect())
}

// Simple fuzzy matching: allows for 1-2 character differences
fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    if needle.len() < 3 {
        return false;
    }
    // Check if most characters of needle appear in haystack in order
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
    let threshold = if needle.len() <= 4 {
        needle.len() - 1
    } else {
        needle.len() - 2
    };
    matched >= threshold
}

// GET /api/brands/top?limit=10
async fn top_brands(
    Query(params): Query<LimitParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let limit = params.limit.unwrap_or(10).min(50);
    let mut brands: Vec<&BrandRating> = state.brands.iter().collect();
    brands.sort_by(|a, b| b.overall_score.cmp(&a.overall_score));
    Json(brands.into_iter().take(limit).cloned().collect())
}

// GET /api/brands/worst?limit=10
async fn worst_brands(
    Query(params): Query<LimitParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let limit = params.limit.unwrap_or(10).min(50);
    let mut brands: Vec<&BrandRating> = state.brands.iter().collect();
    brands.sort_by(|a, b| a.overall_score.cmp(&b.overall_score));
    Json(brands.into_iter().take(limit).cloned().collect())
}

// GET /api/brands/compare?brands=zara,hm,patagonia
async fn compare_brands(
    Query(params): Query<CompareParams>,
    state: Arc<AppState>,
) -> Json<Vec<BrandRating>> {
    let slugs = match params.brands {
        Some(ref b) => b.split(',').map(|s| s.trim().to_lowercase()).collect::<Vec<_>>(),
        None => return Json(vec![]),
    };

    let results: Vec<BrandRating> = slugs
        .iter()
        .filter_map(|slug| state.brands.iter().find(|b| b.slug == *slug))
        .cloned()
        .collect();

    Json(results)
}

// GET /api/categories
async fn get_categories(state: Arc<AppState>) -> Json<Vec<CategoryStats>> {
    let mut category_map: std::collections::HashMap<String, Vec<&BrandRating>> =
        std::collections::HashMap::new();

    for brand in &state.brands {
        category_map
            .entry(brand.category.clone())
            .or_default()
            .push(brand);
    }

    let mut categories: Vec<CategoryStats> = category_map
        .into_iter()
        .map(|(category, brands)| {
            let count = brands.len();
            let avg = |f: fn(&BrandRating) -> u8| -> f64 {
                brands.iter().map(|b| f(b) as f64).sum::<f64>() / count as f64
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

// GET /api/stats
async fn get_stats(state: Arc<AppState>) -> Json<OverallStats> {
    let brands = &state.brands;
    let total = brands.len();

    let avg_score = brands.iter().map(|b| b.overall_score as f64).sum::<f64>() / total as f64;

    let mut scores: Vec<u8> = brands.iter().map(|b| b.overall_score).collect();
    scores.sort();
    let median = scores[total / 2];

    let mut grade_dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for brand in brands {
        *grade_dist.entry(brand.grade.clone()).or_insert(0) += 1;
    }

    let mut price_dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for brand in brands {
        *price_dist.entry(brand.price_range.clone()).or_insert(0) += 1;
    }

    let countries: std::collections::HashSet<&str> = brands.iter().map(|b| b.country.as_str()).collect();

    // Build category stats
    let mut category_map: std::collections::HashMap<String, Vec<&BrandRating>> =
        std::collections::HashMap::new();
    for brand in brands {
        category_map
            .entry(brand.category.clone())
            .or_default()
            .push(brand);
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
