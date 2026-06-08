use axum::{http::header, response::Html, response::IntoResponse};

pub async fn openapi_json() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/json")], OPENAPI_SPEC)
}

pub async fn swagger_ui() -> Html<&'static str> {
    Html(SWAGGER_HTML)
}

const SWAGGER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>Rewoven API - Docs</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  <style>body{margin:0}.topbar{display:none}</style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.ui = SwaggerUIBundle({ url: '/openapi.json', dom_id: '#swagger-ui', deepLinking: true });
  </script>
</body>
</html>"#;

const OPENAPI_SPEC: &str = r##"{
  "openapi": "3.0.3",
  "info": {
    "title": "Rewoven API",
    "version": "1.0.0",
    "description": "Public REST API for fashion brand sustainability ratings and textile material impact data. Ratings are Rewoven's editorial assessments (opinion). See https://rewovenapp.com/methodology/."
  },
  "servers": [{ "url": "https://api.rewovenapp.com" }],
  "tags": [
    { "name": "Brands" }, { "name": "Materials" }, { "name": "Stats" }, { "name": "Barcode" }, { "name": "System" }
  ],
  "paths": {
    "/health": { "get": { "tags": ["System"], "summary": "Health check", "responses": { "200": { "description": "OK" } } } },
    "/v1/brands": {
      "get": {
        "tags": ["Brands"], "summary": "List brands (paginated, filterable)",
        "parameters": [
          { "name": "page", "in": "query", "schema": { "type": "integer", "default": 1 } },
          { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 50, "maximum": 100 } },
          { "name": "category", "in": "query", "schema": { "type": "string" } },
          { "name": "min_score", "in": "query", "schema": { "type": "integer" } },
          { "name": "max_score", "in": "query", "schema": { "type": "integer" } },
          { "name": "search", "in": "query", "schema": { "type": "string" } },
          { "name": "sort", "in": "query", "schema": { "type": "string", "enum": ["score_asc","score_desc","name_asc","name_desc"] } }
        ],
        "responses": { "200": { "description": "Paginated brands", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/PaginatedBrands" } } } } }
      }
    },
    "/v1/brands/{slug}": {
      "get": {
        "tags": ["Brands"], "summary": "Get a single brand (includes rationale)",
        "parameters": [{ "name": "slug", "in": "path", "required": true, "schema": { "type": "string" } }],
        "responses": {
          "200": { "description": "Brand", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/BrandRating" } } } },
          "404": { "description": "Not found" }
        }
      }
    },
    "/v1/brands/search": {
      "get": { "tags": ["Brands"], "summary": "Fuzzy search brands by name",
        "parameters": [{ "name": "q", "in": "query", "required": true, "schema": { "type": "string" } }],
        "responses": { "200": { "description": "Matches", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/BrandRating" } } } } } } }
    },
    "/v1/brands/top": { "get": { "tags": ["Brands"], "summary": "Highest-rated brands", "parameters": [{ "name": "limit", "in": "query", "schema": { "type": "integer", "default": 10 } }], "responses": { "200": { "description": "Brands" } } } },
    "/v1/brands/worst": { "get": { "tags": ["Brands"], "summary": "Lowest-rated brands", "parameters": [{ "name": "limit", "in": "query", "schema": { "type": "integer", "default": 10 } }], "responses": { "200": { "description": "Brands" } } } },
    "/v1/brands/compare": { "get": { "tags": ["Brands"], "summary": "Compare brands", "parameters": [{ "name": "brands", "in": "query", "required": true, "schema": { "type": "string" }, "description": "comma-separated slugs" }], "responses": { "200": { "description": "Brands" } } } },
    "/v1/brands/{slug}/alternatives": { "get": { "tags": ["Brands"], "summary": "More sustainable alternatives", "parameters": [{ "name": "slug", "in": "path", "required": true, "schema": { "type": "string" } }, { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 5 } }], "responses": { "200": { "description": "Alternatives" } } } },
    "/v1/materials": { "get": { "tags": ["Materials"], "summary": "List textile materials with impact data", "responses": { "200": { "description": "Materials" } } } },
    "/v1/materials/{slug}": { "get": { "tags": ["Materials"], "summary": "Get one material", "parameters": [{ "name": "slug", "in": "path", "required": true, "schema": { "type": "string" } }], "responses": { "200": { "description": "Material" }, "404": { "description": "Not found" } } } },
    "/v1/categories": { "get": { "tags": ["Stats"], "summary": "Category averages", "responses": { "200": { "description": "Categories" } } } },
    "/v1/stats": { "get": { "tags": ["Stats"], "summary": "Dataset statistics", "responses": { "200": { "description": "Stats" } } } },
    "/v1/barcode/{upc}": { "get": { "tags": ["Barcode"], "summary": "Look up a brand by barcode", "parameters": [{ "name": "upc", "in": "path", "required": true, "schema": { "type": "string" } }], "responses": { "200": { "description": "Brand match" }, "404": { "description": "No match" } } } }
  },
  "components": {
    "schemas": {
      "BrandRating": {
        "type": "object",
        "properties": {
          "name": { "type": "string" }, "slug": { "type": "string" },
          "overall_score": { "type": "integer" }, "grade": { "type": "string" },
          "environmental_score": { "type": "integer" }, "labor_score": { "type": "integer" },
          "transparency_score": { "type": "integer" }, "animal_welfare_score": { "type": "integer" },
          "price_range": { "type": "string" }, "country": { "type": "string" }, "category": { "type": "string" },
          "certifications": { "type": "array", "items": { "type": "string" } },
          "summary": { "type": "string" }, "website": { "type": "string" },
          "updated_at": { "type": "string", "description": "When this rating was last reviewed" },
          "sources": { "type": "array", "items": { "type": "string" }, "description": "Citation URLs (when available)" },
          "rationale": { "type": "string", "description": "Per-dimension explanation; only on the single-brand endpoint" }
        }
      },
      "PaginatedBrands": {
        "type": "object",
        "properties": {
          "brands": { "type": "array", "items": { "$ref": "#/components/schemas/BrandRating" } },
          "total": { "type": "integer" }, "page": { "type": "integer" }, "pages": { "type": "integer" }
        }
      }
    }
  }
}"##;
