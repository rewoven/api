use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use crate::db;
use crate::error::AppError;
use crate::models::*;
use crate::state::AppState;

// ─── Levenshtein distance for fuzzy search ───

fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost)
                .min(prev[j + 1] + 1)
                .min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    if needle.len() < 3 { return false; }
    let max_dist = if needle.len() <= 4 { 1 } else { 2 };
    levenshtein(haystack, needle) <= max_dist
}

// ─── Handlers ───

pub async fn health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, AppError> {
    let conn = state.db.get()?;
    let count = db::get_brand_count(&conn)?;
    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        total_brands: count,
    }))
}

pub async fn list_brands(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<Json<PaginatedResponse>, AppError> {
    let conn = state.db.get()?;
    let limit = params.limit.unwrap_or(50).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let (brands, total) = db::list_brands(
        &conn,
        params.category.as_deref(),
        params.min_score,
        params.max_score,
        params.search.as_deref(),
        params.sort.as_deref(),
        limit,
        offset,
    )?;

    let pages = if total == 0 { 1 } else { (total + limit - 1) / limit };
    Ok(Json(PaginatedResponse { brands, total, page, pages }))
}

pub async fn get_brand(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<BrandRating>, AppError> {
    let conn = state.db.get()?;
    let slug_lower = slug.to_lowercase();
    match db::get_brand_by_slug(&conn, &slug_lower)? {
        Some(brand) => Ok(Json(brand)),
        None => Err(AppError::NotFound(format!("Brand '{}' not found", slug))),
    }
}

pub async fn search_brands(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<BrandRating>>, AppError> {
    let query = match params.q {
        Some(q) if !q.is_empty() => q,
        _ => return Ok(Json(vec![])),
    };

    let conn = state.db.get()?;

    // SQL search for exact/contains matches
    let mut results = db::search_brands_sql(&conn, &query)?;

    // Levenshtein fuzzy fallback: if SQL found nothing, scan for close matches
    if results.is_empty() {
        let query_lower = query.to_lowercase();
        let (all, _) = db::list_brands(&conn, None, None, None, None, None, 1000, 0)?;
        let mut fuzzy: Vec<(usize, BrandRating)> = all
            .into_iter()
            .filter(|b| fuzzy_match(&b.name.to_lowercase(), &query_lower))
            .map(|b| {
                let dist = levenshtein(&b.name.to_lowercase(), &query_lower);
                (dist, b)
            })
            .collect();
        fuzzy.sort_by_key(|(dist, _)| *dist);
        results = fuzzy.into_iter().map(|(_, b)| b).collect();
    }

    Ok(Json(results))
}

pub async fn top_brands(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LimitParams>,
) -> Result<Json<Vec<BrandRating>>, AppError> {
    let limit = params.limit.unwrap_or(10).min(50);
    let conn = state.db.get()?;
    let brands = db::top_brands(&conn, limit)?;
    Ok(Json(brands))
}

pub async fn worst_brands(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LimitParams>,
) -> Result<Json<Vec<BrandRating>>, AppError> {
    let limit = params.limit.unwrap_or(10).min(50);
    let conn = state.db.get()?;
    let brands = db::worst_brands(&conn, limit)?;
    Ok(Json(brands))
}

pub async fn compare_brands(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CompareParams>,
) -> Result<Json<Vec<BrandRating>>, AppError> {
    let slugs = match params.brands {
        Some(ref b) => b.split(',').map(|s| s.trim().to_lowercase()).collect::<Vec<_>>(),
        None => return Ok(Json(vec![])),
    };
    let conn = state.db.get()?;
    let brands = db::get_brands_by_slugs(&conn, &slugs)?;
    Ok(Json(brands))
}

pub async fn get_alternatives(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(params): Query<AlternativesParams>,
) -> Result<Json<AlternativesResponse>, AppError> {
    let slug_lower = slug.to_lowercase();
    let conn = state.db.get()?;

    let brand = match db::get_brand_by_slug(&conn, &slug_lower)? {
        Some(b) => b,
        None => return Err(AppError::NotFound(format!("Brand '{}' not found", slug))),
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

    let alts = db::get_alternatives(&conn, &slug_lower, &brand.category, min_score, &price_tiers, limit)?;

    let reason = format!(
        "Showing sustainable alternatives to {} (score: {}/100, grade: {}). These brands score higher on sustainability while offering similar style and price range.",
        brand.name, brand.overall_score, brand.grade
    );

    Ok(Json(AlternativesResponse { original: brand, alternatives: alts, reason }))
}
