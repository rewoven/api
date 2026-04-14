use axum::{extract::State, Json};
use std::sync::Arc;

use crate::db;
use crate::error::AppError;
use crate::models::*;
use crate::state::AppState;

pub async fn get_categories(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CategoryStats>>, AppError> {
    let conn = state.db.get()?;
    let categories = db::get_category_stats(&conn)?;
    Ok(Json(categories))
}

pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OverallStats>, AppError> {
    let conn = state.db.get()?;
    let (total, avg_score, median, grade_dist, price_dist, country_count) =
        db::get_overall_stats(&conn)?;

    if total == 0 {
        return Ok(Json(OverallStats {
            total_brands: 0,
            average_score: 0.0,
            median_score: 0,
            grade_distribution: Default::default(),
            category_count: 0,
            categories: vec![],
            price_range_distribution: Default::default(),
            country_count: 0,
        }));
    }

    let categories = db::get_category_stats(&conn)?;

    Ok(Json(OverallStats {
        total_brands: total,
        average_score: avg_score,
        median_score: median,
        grade_distribution: grade_dist,
        category_count: categories.len(),
        categories,
        price_range_distribution: price_dist,
        country_count,
    }))
}
