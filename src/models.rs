use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Grade computation (derived, never persisted) ───

pub fn compute_grade(score: u8) -> &'static str {
    match score {
        90..=100 => "A+",
        80..=89 => "A",
        70..=79 => "B",
        60..=69 => "C",
        50..=59 => "C-",
        40..=49 => "D",
        30..=39 => "D-",
        20..=29 => "E",
        10..=19 => "F",
        _ => "F-",
    }
}

// ─── Core types ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandRating {
    pub name: String,
    pub slug: String,
    pub overall_score: u8,
    pub grade: String,
    pub environmental_score: u8,
    pub labor_score: u8,
    pub transparency_score: u8,
    pub animal_welfare_score: u8,
    pub price_range: String,
    pub country: String,
    pub category: String,
    pub certifications: Vec<String>,
    pub summary: String,
    pub website: String,
}

#[derive(Serialize, Clone)]
pub struct MaterialImpact {
    pub name: String,
    pub slug: String,
    pub category: String,
    pub co2_kg_per_kg: f64,
    pub water_liters_per_kg: f64,
    pub biodegradable: bool,
    pub recyclable: bool,
    pub sustainability_score: u8,
    pub description: String,
}

// ─── Query parameter types ───

#[derive(Deserialize)]
pub struct ListParams {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub category: Option<String>,
    pub min_score: Option<u8>,
    pub max_score: Option<u8>,
    pub search: Option<String>,
    pub sort: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct LimitParams {
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct CompareParams {
    pub brands: Option<String>,
}

#[derive(Deserialize)]
pub struct AlternativesParams {
    pub limit: Option<usize>,
    pub min_score: Option<u8>,
}

// ─── Response types ───

#[derive(Serialize)]
pub struct AlternativesResponse {
    pub original: BrandRating,
    pub alternatives: Vec<BrandRating>,
    pub reason: String,
}

#[derive(Serialize)]
pub struct PaginatedResponse {
    pub brands: Vec<BrandRating>,
    pub total: usize,
    pub page: usize,
    pub pages: usize,
}

#[derive(Serialize)]
pub struct CategoryStats {
    pub category: String,
    pub count: usize,
    pub average_score: f64,
    pub average_environmental: f64,
    pub average_labor: f64,
    pub average_transparency: f64,
    pub average_animal_welfare: f64,
}

#[derive(Serialize)]
pub struct OverallStats {
    pub total_brands: usize,
    pub average_score: f64,
    pub median_score: u8,
    pub grade_distribution: HashMap<String, usize>,
    pub category_count: usize,
    pub categories: Vec<CategoryStats>,
    pub price_range_distribution: HashMap<String, usize>,
    pub country_count: usize,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub status: u16,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub total_brands: usize,
}
