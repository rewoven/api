# Rewoven API

A fast REST API serving sustainability ratings for 500+ fashion brands

## Running

```bash
cargo run
# or for production:
cargo build --release
./target/release/rewoven-api
```

The server starts on `http://0.0.0.0:3000`

## API Endpoints

### Health Check
```
GET /health
```
Returns API status, version, and total brand count

### List Brands (Paginated)
```
GET /api/brands?page=1&limit=50&category=Fast+Fashion&min_score=0&max_score=100&search=zara&sort=score_desc
```
**Query Parameters:**
- `page` (default: 1) - Page number
- `limit` (default: 50, max: 100) - Items per page
- `category` - Filter by category (e.g. "Fast Fashion", "Luxury", "Sustainable", "Sportswear", "Mid-Range", "Outdoor/Active", "Thrift/Resale")
- `min_score` / `max_score` - Filter by overall score range (0-100)
- `search` - Filter by brand name (substring match)
- `sort` - Sort order: `score_desc`, `score_asc`, `name_asc`, `name_desc`

**Response:**
```json
{
  "brands": [...],
  "total": 505,
  "page": 1,
  "pages": 11
}
```

### Get Brand by Slug
```
GET /api/brands/:slug
```
Returns full brand details. Slug is the URL-friendly brand name (e.g. `patagonia`, `h-and-m`)

### Search Brands
```
GET /api/brands/search?q=zara
```
Fuzzy search by brand name. Returns matches ranked by relevance (exact > starts with > contains > fuzzy)

### Top Rated Brands
```
GET /api/brands/top?limit=10
```
Returns the highest-rated brands sorted by overall score

### Worst Rated Brands
```
GET /api/brands/worst?limit=10
```
Returns the lowest-rated brands sorted by overall score.

### Compare Brands
```
GET /api/brands/compare?brands=zara,patagonia,nike
```
Compare multiple brands side by side. Pass comma separated slugs

### Categories
```
GET /api/categories
```
Lists all categories with average scores across all rating dimensions.

### Statistics
```
GET /api/stats
```
Overall statistics including total brands, average/median scores, grade distribution, category breakdown, and price range distribution.

## Brand Rating Fields

| Field | Type | Description |
|-------|------|-------------|
| name | string | Brand display name |
| slug | string | URL-friendly identifier |
| overall_score | 0-100 | Composite sustainability score |
| grade | A+ to F- | Letter grade based on overall score |
| environmental_score | 0-100 | Environmental impact rating |
| labor_score | 0-100 | Labor practices rating |
| transparency_score | 0-100 | Supply chain transparency rating |
| animal_welfare_score | 0-100 | Animal welfare rating |
| price_range | $ to $$$$ | Price tier |
| country | string | Headquarters country |
| category | string | Brand category |
| certifications | string[] | Sustainability certifications held |
| summary | string | Brief description of sustainability practices |
| website | string | Brand website URL |

## Deploying to VPS

1. Build the release binary:
```bash
cargo build --release
```

2. Copy the binary to your VPS:
```bash
scp target/release/rewoven-api user@your-vps:/opt/rewoven-api/
```

3. Create a systemd service (`/etc/systemd/system/rewoven-api.service`):
```ini
[Unit]
Description=Rewoven Brand Sustainability API
After=network.target

[Service]
Type=simple
ExecStart=/opt/rewoven-api/rewoven-api
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

4. Enable and start:
```bash
sudo systemctl enable rewoven-api
sudo systemctl start rewoven-api
```

5. Set up a reverse proxy with nginx or caddy to expose on port 80/443
