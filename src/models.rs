use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

    #[serde(default)]
    pub updated_at: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rationale: String,
}

fn overall_band(score: u8) -> &'static str {
    match score {
        80..=100 => "an industry leader",
        65..=79 => "strong",
        50..=64 => "moderate",
        30..=49 => "below average",
        _ => "poor",
    }
}

fn environmental_note(score: u8) -> &'static str {
    match score {
        65..=100 => "strong use of lower-impact materials and documented action to cut emissions, water use, and waste",
        50..=64 => "some lower-impact materials or efficiency efforts, with clear room to improve",
        30..=49 => "limited publicly documented action on materials, emissions, water, or waste relative to peers",
        _ => "little publicly available evidence of meaningful action on materials, emissions, water, or waste",
    }
}

fn labor_note(score: u8) -> &'static str {
    match score {
        65..=100 => "well-documented labor standards and supply-chain oversight",
        50..=64 => "some labor commitments with partial supply-chain oversight",
        30..=49 => "limited public disclosure of labor standards or supply-chain auditing",
        _ => {
            "little publicly available evidence of enforced labor standards or independent auditing"
        }
    }
}

fn transparency_note(score: u8) -> &'static str {
    match score {
        65..=100 => "detailed public reporting and supply-chain traceability",
        50..=64 => "partial public reporting on its supply chain",
        30..=49 => "limited public reporting or traceability",
        _ => "minimal public disclosure of its supply chain or practices",
    }
}

fn animal_note(score: u8) -> &'static str {
    match score {
        65..=100 => "clear animal-welfare policies or avoidance of high-harm materials",
        50..=64 => "some animal-welfare policies in place",
        30..=49 => "limited animal-welfare policy disclosure",
        _ => "little publicly available animal-welfare policy",
    }
}

pub fn build_rationale(
    name: &str,
    overall: u8,
    environmental: u8,
    labor: u8,
    transparency: u8,
    animal_welfare: u8,
    category: &str,
    certifications: &[String],
    summary: &str,
) -> String {
    let grade = compute_grade(overall);
    let certs_line = if certifications.is_empty() {
        "No third-party sustainability certifications are recorded for this brand.".to_string()
    } else {
        format!(
            "Recognised certifications in our records: {}.",
            certifications.join(", ")
        )
    };

    format!(
        "Rewoven rates {name} {overall}/100 (grade {grade}) - {band} overall for sustainability. \
This composite is the equal-weighted average of four dimensions:\n\n\
• Environmental ({environmental}/100): {env}.\n\
• Labor practices ({labor}/100): {lab}.\n\
• Transparency ({transparency}/100): {trans}.\n\
• Animal welfare ({animal_welfare}/100): {anim}.\n\n\
{certs_line} Assessed within the {category} category. {summary}\n\n\
This rating is Rewoven's editorial assessment based on publicly available information and may not reflect a brand's most recent changes. \
It is an expression of opinion, not a statement of fact, and is provided for general information only. \
Brands may request a review at arhan@rewovenapp.com.",
        name = name,
        overall = overall,
        grade = grade,
        band = overall_band(overall),
        environmental = environmental,
        env = environmental_note(environmental),
        labor = labor,
        lab = labor_note(labor),
        transparency = transparency,
        trans = transparency_note(transparency),
        animal_welfare = animal_welfare,
        anim = animal_note(animal_welfare),
        certs_line = certs_line,
        category = category,
        summary = summary,
    )
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
    pub grade_distribution: BTreeMap<String, usize>,
    pub category_count: usize,
    pub categories: Vec<CategoryStats>,
    pub price_range_distribution: BTreeMap<String, usize>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_boundaries() {
        assert_eq!(compute_grade(100), "A+");
        assert_eq!(compute_grade(90), "A+");
        assert_eq!(compute_grade(89), "A");
        assert_eq!(compute_grade(35), "D-");
        assert_eq!(compute_grade(0), "F-");
    }

    #[test]
    fn rationale_is_grounded_and_disclaimed() {
        let r = build_rationale(
            "Acme",
            35,
            30,
            30,
            36,
            44,
            "Luxury",
            &["B Corp".to_string()],
            "Some summary.",
        );
        assert!(r.contains("Acme"));
        assert!(r.contains("35/100"));
        assert!(r.contains("Luxury"));
        assert!(r.contains("B Corp"));
        assert!(r.contains("opinion"));
        assert!(r.contains("arhan@rewovenapp.com"));
    }

    #[test]
    fn rationale_handles_no_certifications() {
        let r = build_rationale("X", 50, 50, 50, 50, 50, "Mid-Range", &[], "s");
        assert!(r.contains("No third-party"));
    }

    #[test]
    fn list_payload_omits_empty_rationale_and_sources() {
        let b = BrandRating {
            name: "X".into(),
            slug: "x".into(),
            overall_score: 50,
            grade: "C-".into(),
            environmental_score: 50,
            labor_score: 50,
            transparency_score: 50,
            animal_welfare_score: 50,
            price_range: "$$".into(),
            country: "US".into(),
            category: "Mid-Range".into(),
            certifications: vec![],
            summary: "s".into(),
            website: "w".into(),
            updated_at: String::new(),
            sources: vec![],
            rationale: String::new(),
        };
        let json = serde_json::to_string(&b).unwrap();
        assert!(!json.contains("rationale"));
        assert!(!json.contains("sources"));
    }
}
