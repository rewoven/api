use axum::{extract::State, http::header, response::IntoResponse, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::db;
use crate::error::AppError;
use crate::models::BrandRating;
use crate::state::AppState;

const LICENSE: &str = "CC BY 4.0";
const ATTRIBUTION: &str = "Rewoven (rewovenapp.com)";

#[derive(Serialize)]
pub struct ExportResponse {
    pub license: &'static str,
    pub license_url: &'static str,
    pub attribution: &'static str,
    pub source: &'static str,
    pub disclaimer: &'static str,
    pub count: usize,
    pub brands: Vec<BrandRating>,
}

pub async fn export_json(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ExportResponse>, AppError> {
    let conn = state.db.get()?;
    let (brands, total) =
        db::list_brands(&conn, None, None, None, None, Some("name_asc"), 10000, 0)?;
    Ok(Json(ExportResponse {
        license: LICENSE,
        license_url: "https://creativecommons.org/licenses/by/4.0/",
        attribution: ATTRIBUTION,
        source: "https://api.rewovenapp.com",
        disclaimer: "Ratings are Rewoven's editorial assessments (expressions of opinion, not statements of fact). Methodology: https://rewovenapp.com/methodology/",
        count: total,
        brands,
    }))
}

fn csv_field(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub async fn export_csv(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    let conn = state.db.get()?;
    let (brands, _) = db::list_brands(&conn, None, None, None, None, Some("name_asc"), 10000, 0)?;

    let mut out = String::with_capacity(brands.len() * 160);
    out.push_str("name,slug,overall_score,grade,environmental_score,labor_score,transparency_score,animal_welfare_score,price_range,country,category,certifications,summary,website\n");
    for b in &brands {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_field(&b.name),
            csv_field(&b.slug),
            b.overall_score,
            csv_field(&b.grade),
            b.environmental_score,
            b.labor_score,
            b.transparency_score,
            b.animal_welfare_score,
            csv_field(&b.price_range),
            csv_field(&b.country),
            csv_field(&b.category),
            csv_field(&b.certifications.join("; ")),
            csv_field(&b.summary),
            csv_field(&b.website),
        ));
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"rewoven-brand-ratings.csv\"",
            ),
            (header::HeaderName::from_static("x-license"), "CC BY 4.0"),
            (
                header::HeaderName::from_static("x-attribution"),
                "Rewoven (rewovenapp.com)",
            ),
        ],
        out,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_field_escapes_quotes_and_commas() {
        assert_eq!(csv_field("plain"), "\"plain\"");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
