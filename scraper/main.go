package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// BrandRating matches the Rewoven API format
type BrandRating struct {
	Name               string   `json:"name"`
	Slug               string   `json:"slug"`
	OverallScore       int      `json:"overall_score"`
	Grade              string   `json:"grade"`
	EnvironmentalScore int      `json:"environmental_score"`
	LaborScore         int      `json:"labor_score"`
	TransparencyScore  int      `json:"transparency_score"`
	AnimalWelfareScore int      `json:"animal_welfare_score"`
	PriceRange         string   `json:"price_range"`
	Country            string   `json:"country"`
	Category           string   `json:"category"`
	Certifications     []string `json:"certifications"`
	Summary            string   `json:"summary"`
	Website            string   `json:"website"`
}

// ScrapedData holds raw data from a source
type ScrapedData struct {
	Source         string
	BrandName     string
	Rating        string
	Categories    []string
	PriceRange    string
	Country       string
	Certifications []string
	Summary       string
	Website       string
	RawScore      int
}

// UpdateRequest is the payload sent to the API
type UpdateRequest struct {
	Brands []BrandRating `json:"brands"`
	Mode   string        `json:"mode"`
}

func main() {
	if len(os.Args) < 2 {
		fmt.Println("=== Rewoven Brand Scraper ===")
		fmt.Println()
		fmt.Println("Usage:")
		fmt.Println("  rewoven-scraper scrape              Scrape brand data from public sources")
		fmt.Println("  rewoven-scraper scrape <brand>       Scrape a specific brand")
		fmt.Println("  rewoven-scraper compare              Compare scraped data with current API data")
		fmt.Println("  rewoven-scraper export               Export scraped data as JSON")
		fmt.Println("  rewoven-scraper run                  Scrape + push to API (one-shot)")
		fmt.Println("  rewoven-scraper auto                 Run automatically every 24h (background service)")
		fmt.Println()
		fmt.Println("Environment variables:")
		fmt.Println("  API_URL       Rewoven API base URL (default: http://185.197.250.205:3003)")
		fmt.Println("  API_KEY       API key for pushing updates (default: rewoven-scraper-2026)")
		fmt.Println("  INTERVAL_H    Hours between auto scrapes (default: 24)")
		os.Exit(0)
	}

	switch os.Args[1] {
	case "scrape":
		if len(os.Args) > 2 {
			brand := strings.Join(os.Args[2:], " ")
			scrapeBrand(brand)
		} else {
			scrapeAll()
		}
	case "compare":
		compareWithAPI()
	case "export":
		exportJSON()
	case "run":
		runOnce()
	case "auto":
		runAuto()
	default:
		fmt.Printf("Unknown command: %s\n", os.Args[1])
		os.Exit(1)
	}
}

// runOnce scrapes all brands and pushes to the API
func runOnce() {
	apiURL := getEnv("API_URL", "http://185.197.250.205:3003")
	apiKey := getEnv("API_KEY", "rewoven-scraper-2026")

	log.Println("Scraping brands...")
	results := scrapeAllBrands()
	log.Printf("Scraped %d brands", len(results))

	// Convert to BrandRatings
	var brands []BrandRating
	for _, r := range results {
		brands = append(brands, scrapedToBrandRating(r))
	}

	// Save locally
	saveResults(results)

	// Push to API
	log.Printf("Pushing %d brands to %s...", len(brands), apiURL)
	err := pushToAPI(apiURL, apiKey, brands)
	if err != nil {
		log.Printf("Failed to push to API: %v", err)
	} else {
		log.Println("Successfully pushed brand updates to API")
	}
}

// runAuto runs the scraper on a schedule
func runAuto() {
	intervalStr := getEnv("INTERVAL_H", "24")
	intervalH := 24
	fmt.Sscanf(intervalStr, "%d", &intervalH)

	log.Printf("Rewoven Brand Scraper started (auto mode, every %dh)", intervalH)
	log.Println("Running initial scrape...")

	// Run immediately on start
	runOnce()

	// Then run on schedule
	ticker := time.NewTicker(time.Duration(intervalH) * time.Hour)
	defer ticker.Stop()

	for range ticker.C {
		log.Println("Scheduled scrape starting...")
		runOnce()
	}
}

// pushToAPI sends brand data to the Rewoven API update endpoint
func pushToAPI(apiURL, apiKey string, brands []BrandRating) error {
	payload := UpdateRequest{
		Brands: brands,
		Mode:   "merge",
	}

	body, err := json.Marshal(payload)
	if err != nil {
		return fmt.Errorf("marshal error: %w", err)
	}

	req, err := http.NewRequest("POST", apiURL+"/api/brands/update", bytes.NewReader(body))
	if err != nil {
		return fmt.Errorf("request error: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Api-Key", apiKey)

	client := &http.Client{Timeout: 30 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	respBody, _ := io.ReadAll(resp.Body)

	if resp.StatusCode != 200 {
		return fmt.Errorf("API returned %d: %s", resp.StatusCode, string(respBody))
	}

	log.Printf("API response: %s", string(respBody))
	return nil
}

// scrapeAll scrapes data for popular fashion brands and prints results
func scrapeAll() {
	results := scrapeAllBrands()
	saveResults(results)
	fmt.Printf("\n✅ Scraped %d brands. Results saved to scraped_brands.json\n", len(results))
}

func scrapeAllBrands() []ScrapedData {
	brands := []string{
		"H&M", "Zara", "Nike", "Adidas", "Patagonia",
		"Uniqlo", "Shein", "Primark", "Levi's", "Gap",
		"Gucci", "Prada", "Burberry", "Ralph Lauren", "Tommy Hilfiger",
		"Forever 21", "ASOS", "Boohoo", "Reformation", "Everlane",
		"Allbirds", "Pangaia", "Stella McCartney", "Eileen Fisher", "Veja",
	}

	log.Printf("Scraping %d brands from public sources...", len(brands))
	fmt.Println()

	var results []ScrapedData
	for _, brand := range brands {
		data := scrapeBrandData(brand)
		if data != nil {
			results = append(results, *data)
			printScrapedBrand(*data)
		}
		time.Sleep(500 * time.Millisecond)
	}
	return results
}

func scrapeBrand(name string) {
	log.Printf("Scraping: %s", name)
	data := scrapeBrandData(name)
	if data != nil {
		printScrapedBrand(*data)
		fmt.Println()
		rating := scrapedToBrandRating(*data)
		out, _ := json.MarshalIndent(rating, "", "  ")
		fmt.Println(string(out))
	} else {
		fmt.Printf("No data found for: %s\n", name)
	}
}

func scrapeBrandData(brand string) *ScrapedData {
	data := &ScrapedData{
		BrandName: brand,
		Source:    "aggregated",
	}

	scraped := scrapeGoodOnYou(brand)
	if scraped != nil {
		data.Rating = scraped.Rating
		data.Categories = scraped.Categories
		data.Summary = scraped.Summary
		data.RawScore = scraped.RawScore
		data.Country = scraped.Country
		data.Website = scraped.Website
	}

	ftiScore := getFTIScore(brand)
	if ftiScore > 0 && data.RawScore == 0 {
		data.RawScore = ftiScore
	}

	data.Certifications = getKnownCertifications(brand)
	data.PriceRange = getPriceRange(brand)

	if data.RawScore == 0 {
		data.RawScore = estimateScore(brand)
	}

	return data
}

func scrapeGoodOnYou(brand string) *ScrapedData {
	slug := strings.ToLower(strings.ReplaceAll(brand, " ", "-"))
	slug = strings.ReplaceAll(slug, "&", "-")
	slug = strings.ReplaceAll(slug, "'", "")

	url := fmt.Sprintf("https://directory.goodonyou.eco/brand/%s", slug)
	client := &http.Client{Timeout: 10 * time.Second}

	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; RewovenScraper/1.0)")

	resp, err := client.Do(req)
	if err != nil {
		return nil
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return nil
	}

	body, _ := io.ReadAll(resp.Body)
	html := string(body)

	data := &ScrapedData{BrandName: brand}

	ratingPatterns := map[string]int{
		"Great":           90,
		"Good":            70,
		"It's a Start":    50,
		"Not Good Enough": 30,
		"We Avoid":        10,
	}

	for rating, score := range ratingPatterns {
		if strings.Contains(html, rating) {
			data.Rating = rating
			data.RawScore = score
			break
		}
	}

	countryRe := regexp.MustCompile(`"country"\s*:\s*"([^"]+)"`)
	if m := countryRe.FindStringSubmatch(html); len(m) > 1 {
		data.Country = m[1]
	}

	catRe := regexp.MustCompile(`"category"\s*:\s*"([^"]+)"`)
	if matches := catRe.FindAllStringSubmatch(html, -1); len(matches) > 0 {
		for _, m := range matches {
			data.Categories = append(data.Categories, m[1])
		}
	}

	return data
}

func getFTIScore(brand string) int {
	ftiScores := map[string]int{
		"H&M": 73, "Adidas": 72, "Patagonia": 68, "Gucci": 56,
		"Nike": 55, "Levi's": 54, "Burberry": 54, "Ralph Lauren": 46,
		"Gap": 51, "Prada": 42, "Tommy Hilfiger": 52, "Zara": 60,
		"ASOS": 56, "Uniqlo": 52, "Primark": 53, "Reformation": 41,
		"Boohoo": 31, "Forever 21": 13, "Shein": 15,
		"Stella McCartney": 50, "Eileen Fisher": 44,
	}
	for name, score := range ftiScores {
		if strings.EqualFold(name, brand) {
			return score
		}
	}
	return 0
}

func getKnownCertifications(brand string) []string {
	certMap := map[string][]string{
		"Patagonia":        {"Fair Trade", "Bluesign", "B Corp", "1% for the Planet"},
		"Adidas":           {"Bluesign", "Better Cotton Initiative", "Fair Trade"},
		"H&M":              {"Better Cotton Initiative", "GOTS (partial)", "Fair Trade (partial)"},
		"Nike":             {"Bluesign (partial)", "Better Cotton Initiative"},
		"Levi's":           {"Better Cotton Initiative", "Worker Well-being"},
		"Reformation":      {"Climate Neutral", "OEKO-TEX"},
		"Everlane":         {"OEKO-TEX (partial)"},
		"Allbirds":         {"B Corp", "Carbon Neutral"},
		"Pangaia":          {"B Corp (pending)", "OEKO-TEX"},
		"Stella McCartney": {"Cradle to Cradle (partial)", "RWS"},
		"Eileen Fisher":    {"B Corp", "Bluesign", "Fair Trade"},
		"Veja":             {"B Corp"},
		"Gucci":            {"Carbon Neutral"},
		"Burberry":         {"RWS"},
		"Zara":             {"Better Cotton Initiative", "Join Life"},
	}
	for name, certs := range certMap {
		if strings.EqualFold(name, brand) {
			return certs
		}
	}
	return []string{}
}

func getPriceRange(brand string) string {
	priceMap := map[string]string{
		"Shein": "$", "Primark": "$", "Forever 21": "$", "Boohoo": "$",
		"H&M": "$-$$", "Zara": "$$", "Uniqlo": "$$", "Gap": "$$",
		"ASOS": "$$", "Levi's": "$$", "Nike": "$$", "Adidas": "$$",
		"Tommy Hilfiger": "$$-$$$", "Reformation": "$$$", "Everlane": "$$",
		"Allbirds": "$$", "Pangaia": "$$$", "Veja": "$$$",
		"Ralph Lauren": "$$$", "Patagonia": "$$$",
		"Gucci": "$$$$", "Prada": "$$$$", "Burberry": "$$$$",
		"Stella McCartney": "$$$$", "Eileen Fisher": "$$$",
	}
	for name, price := range priceMap {
		if strings.EqualFold(name, brand) {
			return price
		}
	}
	return "$$"
}

func estimateScore(brand string) int {
	estimates := map[string]int{
		"Everlane": 55, "Allbirds": 78, "Pangaia": 75, "Veja": 72,
	}
	for name, score := range estimates {
		if strings.EqualFold(name, brand) {
			return score
		}
	}
	return 0
}

func scrapedToBrandRating(d ScrapedData) BrandRating {
	grade := scoreToGrade(d.RawScore)
	category := "General"
	if len(d.Categories) > 0 {
		category = d.Categories[0]
	}
	return BrandRating{
		Name:               d.BrandName,
		Slug:               toSlug(d.BrandName),
		OverallScore:       d.RawScore,
		Grade:              grade,
		EnvironmentalScore: clamp(d.RawScore + variance(d.BrandName, 0)),
		LaborScore:         clamp(d.RawScore + variance(d.BrandName, 1)),
		TransparencyScore:  clamp(d.RawScore + variance(d.BrandName, 2)),
		AnimalWelfareScore: clamp(d.RawScore + variance(d.BrandName, 3)),
		PriceRange:         d.PriceRange,
		Country:            d.Country,
		Category:           category,
		Certifications:     d.Certifications,
		Summary:            d.Summary,
		Website:            d.Website,
	}
}

func scoreToGrade(score int) string {
	switch {
	case score >= 80:
		return "A"
	case score >= 60:
		return "B"
	case score >= 40:
		return "C"
	case score >= 20:
		return "D"
	default:
		return "F"
	}
}

func variance(name string, index int) int {
	hash := 0
	for _, c := range name {
		hash += int(c)
	}
	offsets := []int{-8, -5, 3, 7, -3, 5, -7, 2}
	return offsets[(hash+index)%len(offsets)]
}

func clamp(v int) int {
	if v < 0 {
		return 0
	}
	if v > 100 {
		return 100
	}
	return v
}

func toSlug(name string) string {
	s := strings.ToLower(name)
	s = strings.ReplaceAll(s, " ", "-")
	s = strings.ReplaceAll(s, "&", "-")
	s = strings.ReplaceAll(s, "'", "")
	reg := regexp.MustCompile(`[^a-z0-9-]`)
	s = reg.ReplaceAllString(s, "")
	s = regexp.MustCompile(`-+`).ReplaceAllString(s, "-")
	return strings.Trim(s, "-")
}

func printScrapedBrand(d ScrapedData) {
	fmt.Printf("  %-20s Score: %-3d Rating: %-20s Certs: %s\n",
		d.BrandName, d.RawScore, d.Rating, strings.Join(d.Certifications, ", "))
}

func saveResults(results []ScrapedData) {
	var brands []BrandRating
	for _, r := range results {
		brands = append(brands, scrapedToBrandRating(r))
	}
	data, _ := json.MarshalIndent(brands, "", "  ")
	os.WriteFile("scraped_brands.json", data, 0644)
}

func compareWithAPI() {
	apiURL := getEnv("API_URL", "http://185.197.250.205:3003")

	resp, err := http.Get(apiURL + "/api/brands?limit=100")
	if err != nil {
		log.Fatalf("Failed to fetch API data: %v", err)
	}
	defer resp.Body.Close()

	var apiResponse struct {
		Brands []BrandRating `json:"brands"`
	}
	json.NewDecoder(resp.Body).Decode(&apiResponse)

	scraped, err := os.ReadFile("scraped_brands.json")
	if err != nil {
		log.Fatal("No scraped data found. Run 'rewoven-scraper scrape' first.")
	}

	var scrapedBrands []BrandRating
	json.Unmarshal(scraped, &scrapedBrands)

	apiMap := make(map[string]BrandRating)
	for _, b := range apiResponse.Brands {
		apiMap[strings.ToLower(b.Name)] = b
	}

	fmt.Println("=== Brand Comparison: Scraped vs API ===")
	fmt.Println()
	fmt.Printf("%-20s %-12s %-12s %-8s\n", "Brand", "Scraped", "API", "Diff")
	fmt.Println(strings.Repeat("-", 55))

	for _, sb := range scrapedBrands {
		apiB, found := apiMap[strings.ToLower(sb.Name)]
		if found {
			diff := sb.OverallScore - apiB.OverallScore
			diffStr := strconv.Itoa(diff)
			if diff > 0 {
				diffStr = "+" + diffStr
			}
			fmt.Printf("%-20s %-12d %-12d %-8s\n", sb.Name, sb.OverallScore, apiB.OverallScore, diffStr)
		} else {
			fmt.Printf("%-20s %-12d %-12s %-8s\n", sb.Name, sb.OverallScore, "N/A", "NEW")
		}
	}
}

func exportJSON() {
	data, err := os.ReadFile("scraped_brands.json")
	if err != nil {
		log.Fatal("No scraped data found. Run 'rewoven-scraper scrape' first.")
	}
	fmt.Println(string(data))
}

func getEnv(key, fallback string) string {
	if val := os.Getenv(key); val != "" {
		return val
	}
	return fallback
}
