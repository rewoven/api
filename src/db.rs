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

        -- ── Barcode → brand mapping ──────────────────────────────────
        -- The first 6-9 digits of a UPC/EAN are the GS1 company prefix
        -- assigned to the manufacturer. We map prefix → brand_slug so
        -- /api/barcode/{upc} can look up the brand without an external API.
        --
        -- source:    'manual'         curated by us, high confidence
        --            'gs1'            looked up from GS1's public registry
        --            'crowdsourced'   submitted by an app user
        --            'partner'        added via partner integration
        --
        -- confidence: 0-100 — we only return matches with confidence >= 50
        --             so a single crowdsourced report can't pollute the data.
        CREATE TABLE IF NOT EXISTS barcode_prefixes (
            prefix TEXT PRIMARY KEY,
            brand_slug TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            confidence INTEGER NOT NULL DEFAULT 100,
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (brand_slug) REFERENCES brands(slug)
        );
        CREATE INDEX IF NOT EXISTS idx_barcode_prefixes_brand ON barcode_prefixes(brand_slug);

        -- A staging table for crowdsourced submissions that haven't been
        -- promoted to the main table yet. Same prefix needs N reports
        -- before we add it to barcode_prefixes with confidence 70.
        CREATE TABLE IF NOT EXISTS barcode_submissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            prefix TEXT NOT NULL,
            brand_slug TEXT NOT NULL,
            user_hash TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (brand_slug) REFERENCES brands(slug)
        );
        CREATE INDEX IF NOT EXISTS idx_submissions_prefix ON barcode_submissions(prefix);
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

// ─── Barcode prefix lookup ──────────────────────────────────────────

/// Try matching a barcode against our prefix table. We strip non-digit
/// characters (some scanners emit hyphens), then walk longest-to-shortest
/// prefixes (9 → 6 digits) and return the first match with confidence
/// >= 50. Returns the brand slug.
pub fn find_brand_by_barcode(
    conn: &Connection,
    raw_barcode: &str,
) -> Result<Option<(String, String, i64)>, rusqlite::Error> {
    let digits: String = raw_barcode.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 6 {
        return Ok(None);
    }

    // Try longest GS1 prefixes first (most specific wins)
    for len in [9, 8, 7, 6].iter() {
        if digits.len() < *len {
            continue;
        }
        let prefix = &digits[..*len];

        let row: Option<(String, String, i64)> = conn
            .query_row(
                "SELECT brand_slug, source, confidence FROM barcode_prefixes
                 WHERE prefix = ?1 AND confidence >= 50",
                params![prefix],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();

        if let Some(hit) = row {
            return Ok(Some(hit));
        }
    }
    Ok(None)
}

/// Returns total prefix count for /health-style reporting.
pub fn get_barcode_prefix_count(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM barcode_prefixes", [], |r| r.get::<_, i64>(0))
        .map(|n| n as usize)
}

/// Bulk-insert prefix → brand mappings. Idempotent: existing prefixes
/// are left untouched (so re-seeding doesn't downgrade crowdsourced
/// data that's been promoted to the main table).
pub fn seed_barcode_prefixes(
    conn: &Connection,
    prefixes: &[(&str, &str, &str)], // (prefix, brand_slug, notes)
) -> Result<usize, rusqlite::Error> {
    let mut inserted = 0;
    for (prefix, slug, notes) in prefixes {
        // Skip if the brand isn't in our brands table — keeps FK clean
        let brand_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM brands WHERE slug = ?1",
            params![slug],
            |r| r.get(0),
        )?;
        if brand_exists == 0 {
            tracing::warn!("Skipping barcode prefix {} — brand '{}' not in DB", prefix, slug);
            continue;
        }

        let res = conn.execute(
            "INSERT OR IGNORE INTO barcode_prefixes (prefix, brand_slug, source, confidence, notes)
             VALUES (?1, ?2, 'manual', 100, ?3)",
            params![prefix, slug, notes],
        )?;
        inserted += res;
    }
    Ok(inserted)
}

/// Record a crowdsourced submission. After 3 matching submissions for
/// the same (prefix, brand), promote to the main table at confidence 70.
pub fn submit_crowdsourced_prefix(
    conn: &Connection,
    prefix: &str,
    brand_slug: &str,
    user_hash: Option<&str>,
) -> Result<(), rusqlite::Error> {
    // Verify brand exists before recording
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM brands WHERE slug = ?1",
        params![brand_slug],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "Unknown brand_slug: {}",
            brand_slug
        )));
    }

    conn.execute(
        "INSERT INTO barcode_submissions (prefix, brand_slug, user_hash) VALUES (?1, ?2, ?3)",
        params![prefix, brand_slug, user_hash],
    )?;

    // If 3+ users have agreed AND this prefix isn't already in the main
    // table at high confidence, promote it.
    let agreement_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT COALESCE(user_hash, id)) FROM barcode_submissions
         WHERE prefix = ?1 AND brand_slug = ?2",
        params![prefix, brand_slug],
        |r| r.get(0),
    )?;

    if agreement_count >= 3 {
        conn.execute(
            "INSERT OR REPLACE INTO barcode_prefixes
             (prefix, brand_slug, source, confidence, notes)
             VALUES (?1, ?2, 'crowdsourced', 70, 'auto-promoted from user submissions')",
            params![prefix, brand_slug],
        )?;
    }

    Ok(())
}
