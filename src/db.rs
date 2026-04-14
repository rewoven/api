use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::models::{BrandRating, CategoryStats, compute_grade};

// ─── Init & Seed ───

pub fn init_db(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS brands (
            slug TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            overall_score INTEGER NOT NULL,
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
        CREATE INDEX IF NOT EXISTS idx_brands_category ON brands(category);
        CREATE INDEX IF NOT EXISTS idx_brands_score ON brands(overall_score);
        CREATE INDEX IF NOT EXISTS idx_brands_name ON brands(name COLLATE NOCASE);
    ")?;
    Ok(())
}

pub fn seed_db(conn: &Connection, brands: &[BrandRating]) -> Result<(), rusqlite::Error> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM brands", [], |row| row.get(0))?;

    if count > 0 {
        tracing::info!("Database already has {} brands, skipping seed", count);
        return Ok(());
    }

    tracing::info!("Seeding database with {} brands...", brands.len());
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO brands (slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
    )?;

    for b in brands {
        let certs_json = serde_json::to_string(&b.certifications).unwrap_or_else(|_| "[]".to_string());
        stmt.execute(params![
            b.slug, b.name, b.overall_score,
            b.environmental_score, b.labor_score, b.transparency_score, b.animal_welfare_score,
            b.price_range, b.country, b.category, certs_json, b.summary, b.website
        ])?;
    }
    tracing::info!("Seeded {} brands into database", brands.len());
    Ok(())
}

// ─── Row mapper ───

fn row_to_brand(row: &rusqlite::Row) -> rusqlite::Result<BrandRating> {
    let overall_score: u8 = row.get("overall_score")?;
    let certs_str: String = row.get("certifications")?;
    let certifications: Vec<String> = serde_json::from_str(&certs_str).unwrap_or_default();
    Ok(BrandRating {
        slug: row.get("slug")?,
        name: row.get("name")?,
        overall_score,
        grade: compute_grade(overall_score).to_string(),
        environmental_score: row.get("environmental_score")?,
        labor_score: row.get("labor_score")?,
        transparency_score: row.get("transparency_score")?,
        animal_welfare_score: row.get("animal_welfare_score")?,
        price_range: row.get("price_range")?,
        country: row.get("country")?,
        category: row.get("category")?,
        certifications,
        summary: row.get("summary")?,
        website: row.get("website")?,
    })
}

// ─── Targeted queries ───

pub fn get_brand_by_slug(conn: &Connection, slug: &str) -> Result<Option<BrandRating>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website FROM brands WHERE slug = ?1"
    )?;
    let mut rows = stmt.query_map(params![slug], row_to_brand)?;
    match rows.next() {
        Some(Ok(brand)) => Ok(Some(brand)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

pub fn get_brand_count(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM brands", [], |row| row.get(0))
}

pub fn list_brands(
    conn: &Connection,
    category: Option<&str>,
    min_score: Option<u8>,
    max_score: Option<u8>,
    search: Option<&str>,
    sort: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<BrandRating>, usize), rusqlite::Error> {
    let mut where_clauses = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(cat) = category {
        where_clauses.push("LOWER(category) = LOWER(?)");
        param_values.push(Box::new(cat.to_string()));
    }
    if let Some(min) = min_score {
        where_clauses.push("overall_score >= ?");
        param_values.push(Box::new(min));
    }
    if let Some(max) = max_score {
        where_clauses.push("overall_score <= ?");
        param_values.push(Box::new(max));
    }
    if let Some(s) = search {
        where_clauses.push("LOWER(name) LIKE ?");
        param_values.push(Box::new(format!("%{}%", s.to_lowercase())));
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    let order_sql = match sort {
        Some("score_desc") => "ORDER BY overall_score DESC",
        Some("score_asc") => "ORDER BY overall_score ASC",
        Some("name_asc") => "ORDER BY name COLLATE NOCASE ASC",
        Some("name_desc") => "ORDER BY name COLLATE NOCASE DESC",
        _ => "",
    };

    // Get total count
    let count_sql = format!("SELECT COUNT(*) FROM brands {}", where_sql);
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let total: usize = conn.query_row(&count_sql, params_ref.as_slice(), |row| row.get(0))?;

    // Get paginated results
    let query_sql = format!(
        "SELECT slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website FROM brands {} {} LIMIT ? OFFSET ?",
        where_sql, order_sql
    );
    let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for p in &param_values {
        // Re-create params for the second query
        all_params.push(Box::new(p.as_ref().to_sql().unwrap()));
    }

    // Build params including limit and offset
    let mut final_params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let limit_val = limit as i64;
    let offset_val = offset as i64;
    final_params.push(&limit_val);
    final_params.push(&offset_val);

    let mut stmt = conn.prepare(&query_sql)?;
    let brands: Vec<BrandRating> = stmt
        .query_map(final_params.as_slice(), row_to_brand)?
        .filter_map(|r| r.ok())
        .collect();

    Ok((brands, total))
}

pub fn top_brands(conn: &Connection, limit: usize) -> Result<Vec<BrandRating>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website FROM brands ORDER BY overall_score DESC LIMIT ?1"
    )?;
    let brands = stmt.query_map(params![limit as i64], row_to_brand)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(brands)
}

pub fn worst_brands(conn: &Connection, limit: usize) -> Result<Vec<BrandRating>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website FROM brands ORDER BY overall_score ASC LIMIT ?1"
    )?;
    let brands = stmt.query_map(params![limit as i64], row_to_brand)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(brands)
}

pub fn get_brands_by_slugs(conn: &Connection, slugs: &[String]) -> Result<Vec<BrandRating>, rusqlite::Error> {
    if slugs.is_empty() {
        return Ok(vec![]);
    }
    let placeholders: Vec<String> = (1..=slugs.len()).map(|i| format!("?{}", i)).collect();
    let sql = format!(
        "SELECT slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website FROM brands WHERE slug IN ({})",
        placeholders.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = slugs.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
    let brands = stmt.query_map(params.as_slice(), row_to_brand)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(brands)
}

pub fn search_brands_sql(conn: &Connection, query: &str) -> Result<Vec<BrandRating>, rusqlite::Error> {
    let pattern = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website FROM brands WHERE LOWER(name) LIKE ?1 ORDER BY CASE WHEN LOWER(name) = ?2 THEN 0 WHEN LOWER(name) LIKE ?3 THEN 1 ELSE 2 END, overall_score DESC"
    )?;
    let starts_with = format!("{}%", query.to_lowercase());
    let brands = stmt.query_map(params![pattern, query.to_lowercase(), starts_with], row_to_brand)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(brands)
}

pub fn get_alternatives(
    conn: &Connection,
    slug: &str,
    category: &str,
    min_score: u8,
    price_tiers: &[&str],
    limit: usize,
) -> Result<Vec<BrandRating>, rusqlite::Error> {
    let price_placeholders: Vec<String> = (0..price_tiers.len())
        .map(|i| format!("?{}", i + 4))
        .collect();
    let price_in = if price_placeholders.is_empty() {
        "0".to_string()
    } else {
        format!("price_range IN ({})", price_placeholders.join(", "))
    };

    let sql = format!(
        "SELECT slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website,
         (CASE WHEN LOWER(category) = LOWER(?1) THEN 100 ELSE 0 END +
          CASE WHEN {} THEN 50 ELSE 0 END +
          overall_score) AS relevance
         FROM brands
         WHERE slug != ?2 AND overall_score >= ?3
         ORDER BY relevance DESC
         LIMIT ?{}",
        price_in,
        price_tiers.len() + 4
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params_vec.push(Box::new(category.to_string()));
    params_vec.push(Box::new(slug.to_string()));
    params_vec.push(Box::new(min_score));
    for tier in price_tiers {
        params_vec.push(Box::new(tier.to_string()));
    }
    params_vec.push(Box::new(limit as i64));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let brands = stmt.query_map(params_ref.as_slice(), row_to_brand)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(brands)
}

// ─── Stats (aggregated in SQL) ───

pub fn get_category_stats(conn: &Connection) -> Result<Vec<CategoryStats>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT category, COUNT(*) as cnt,
         ROUND(AVG(overall_score), 1) as avg_score,
         ROUND(AVG(environmental_score), 1) as avg_env,
         ROUND(AVG(labor_score), 1) as avg_labor,
         ROUND(AVG(transparency_score), 1) as avg_trans,
         ROUND(AVG(animal_welfare_score), 1) as avg_animal
         FROM brands GROUP BY category ORDER BY avg_score DESC"
    )?;
    let stats = stmt.query_map([], |row| {
        Ok(CategoryStats {
            category: row.get("category")?,
            count: row.get::<_, usize>("cnt")?,
            average_score: row.get("avg_score")?,
            average_environmental: row.get("avg_env")?,
            average_labor: row.get("avg_labor")?,
            average_transparency: row.get("avg_trans")?,
            average_animal_welfare: row.get("avg_animal")?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(stats)
}

pub fn get_overall_stats(conn: &Connection) -> Result<(usize, f64, u8, HashMap<String, usize>, HashMap<String, usize>, usize), rusqlite::Error> {
    let total: usize = get_brand_count(conn)?;
    if total == 0 {
        return Ok((0, 0.0, 0, HashMap::new(), HashMap::new(), 0));
    }

    let avg_score: f64 = conn.query_row(
        "SELECT ROUND(AVG(overall_score), 1) FROM brands", [], |row| row.get(0)
    )?;

    let median: u8 = conn.query_row(
        "SELECT overall_score FROM brands ORDER BY overall_score LIMIT 1 OFFSET ?1",
        params![total / 2],
        |row| row.get(0),
    )?;

    // Grade distribution (computed from score ranges)
    let mut grade_dist: HashMap<String, usize> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT CASE
            WHEN overall_score >= 90 THEN 'A+'
            WHEN overall_score >= 80 THEN 'A'
            WHEN overall_score >= 70 THEN 'B'
            WHEN overall_score >= 60 THEN 'C'
            WHEN overall_score >= 50 THEN 'C-'
            WHEN overall_score >= 40 THEN 'D'
            WHEN overall_score >= 30 THEN 'D-'
            WHEN overall_score >= 20 THEN 'E'
            WHEN overall_score >= 10 THEN 'F'
            ELSE 'F-'
         END as grade, COUNT(*) as cnt FROM brands GROUP BY grade"
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let grade: String = row.get(0)?;
        let count: usize = row.get(1)?;
        grade_dist.insert(grade, count);
    }

    // Price range distribution
    let mut price_dist: HashMap<String, usize> = HashMap::new();
    let mut stmt = conn.prepare("SELECT price_range, COUNT(*) FROM brands GROUP BY price_range")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let range: String = row.get(0)?;
        let count: usize = row.get(1)?;
        price_dist.insert(range, count);
    }

    let country_count: usize = conn.query_row(
        "SELECT COUNT(DISTINCT country) FROM brands", [], |row| row.get(0)
    )?;

    Ok((total, avg_score, median, grade_dist, price_dist, country_count))
}

// ─── Upsert (no wasted SELECT) ───

#[allow(dead_code)]
pub fn upsert_brand(conn: &Connection, b: &BrandRating) -> Result<(), rusqlite::Error> {
    let certs_json = serde_json::to_string(&b.certifications).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO brands (slug, name, overall_score, environmental_score, labor_score, transparency_score, animal_welfare_score, price_range, country, category, certifications, summary, website, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime('now'))
         ON CONFLICT(slug) DO UPDATE SET
            name=excluded.name, overall_score=excluded.overall_score,
            environmental_score=excluded.environmental_score, labor_score=excluded.labor_score,
            transparency_score=excluded.transparency_score, animal_welfare_score=excluded.animal_welfare_score,
            price_range=excluded.price_range, country=excluded.country, category=excluded.category,
            certifications=excluded.certifications, summary=excluded.summary, website=excluded.website,
            updated_at=datetime('now')",
        params![
            b.slug, b.name, b.overall_score,
            b.environmental_score, b.labor_score, b.transparency_score, b.animal_welfare_score,
            b.price_range, b.country, b.category, certs_json, b.summary, b.website
        ],
    )?;
    Ok(())
}

pub fn create_pool(path: &str) -> Result<Pool<SqliteConnectionManager>, r2d2::Error> {
    let manager = SqliteConnectionManager::file(path);
    Pool::builder().max_size(8).build(manager)
}
