use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::AppError;
use crate::state::AppState;
use crate::{db, models::BrandRating};

#[derive(Serialize)]
pub struct BarcodeLookupResponse {
    /// Cleaned (digits only) version of the input
    pub barcode: String,
    /// "ours" if served from our DB, "miss" if no match
    pub source: String,
    /// Provenance of the prefix mapping: 'manual' / 'gs1' / 'crowdsourced' / 'partner'
    pub prefix_source: Option<String>,
    /// 0-100 confidence in the match
    pub confidence: Option<i64>,
    /// Full brand record from our database, when matched
    pub brand: Option<BrandRating>,
}

/// GET /api/barcode/{upc}
///
/// Look up a brand by UPC/EAN barcode. Returns the matching brand if
/// the first 6-9 digits hit a registered prefix in our database.
pub async fn lookup_barcode(
    State(state): State<Arc<AppState>>,
    Path(upc): Path<String>,
) -> Result<Json<BarcodeLookupResponse>, AppError> {
    let conn = state.db.get()?;
    let cleaned: String = upc.chars().filter(|c| c.is_ascii_digit()).collect();

    if cleaned.len() < 6 {
        return Ok(Json(BarcodeLookupResponse {
            barcode: cleaned,
            source: "miss".into(),
            prefix_source: None,
            confidence: None,
            brand: None,
        }));
    }

    match db::find_brand_by_barcode(&conn, &cleaned)? {
        Some((slug, prefix_source, confidence)) => {
            let brand = db::get_brand_by_slug(&conn, &slug)?;
            Ok(Json(BarcodeLookupResponse {
                barcode: cleaned,
                source: "ours".into(),
                prefix_source: Some(prefix_source),
                confidence: Some(confidence),
                brand,
            }))
        }
        None => Ok(Json(BarcodeLookupResponse {
            barcode: cleaned,
            source: "miss".into(),
            prefix_source: None,
            confidence: None,
            brand: None,
        })),
    }
}

#[derive(Deserialize)]
pub struct ContributeRequest {
    /// Full barcode the user scanned. We extract the prefix server-side.
    pub barcode: String,
    /// Slug of the brand the user manually identified after the lookup miss.
    pub brand_slug: String,
    /// Optional anonymous identifier for de-duping submissions per user.
    pub user_hash: Option<String>,
}

#[derive(Serialize)]
pub struct ContributeResponse {
    pub ok: bool,
    pub message: String,
}

/// POST /api/barcode/contribute
///
/// Crowdsourcing endpoint: when a user manually identifies a brand
/// after a lookup miss, the app POSTs the (barcode, brand) pair here.
/// Three independent confirmations promote a prefix into the main
/// `barcode_prefixes` table at confidence 70.
pub async fn contribute_barcode(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ContributeRequest>,
) -> Result<Json<ContributeResponse>, AppError> {
    let conn = state.db.get()?;
    let cleaned: String = body.barcode.chars().filter(|c| c.is_ascii_digit()).collect();

    // Use the 8-digit prefix by default — sweet spot for company specificity
    if cleaned.len() < 8 {
        return Ok(Json(ContributeResponse {
            ok: false,
            message: "Barcode too short (need at least 8 digits)".into(),
        }));
    }
    let prefix = &cleaned[..8];

    db::submit_crowdsourced_prefix(&conn, prefix, &body.brand_slug, body.user_hash.as_deref())
        .map_err(|e| match e {
            rusqlite::Error::InvalidParameterName(msg) => AppError::NotFound(msg),
            other => AppError::Db(other.to_string()),
        })?;

    Ok(Json(ContributeResponse {
        ok: true,
        message: "Thanks — we'll use this to improve our barcode database.".into(),
    }))
}
