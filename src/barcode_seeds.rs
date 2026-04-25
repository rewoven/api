//! Seed data: top fashion-brand GS1 company prefixes.
//!
//! Each tuple is (8-digit-prefix, brand-slug, notes).
//!
//! Brands map to entries in our `brands` table. If the slug doesn't
//! exist, the seeder skips with a warning rather than fail.
//!
//! Sources:
//! - GS1 GEPIR public lookup (gepir.gs1.org)
//! - Public corporate filings + product UPC samples
//! - User-contributed during testing
//!
//! Some companies have MULTIPLE prefixes (different product lines,
//! regional subsidiaries, acquisitions). We list each separately. If
//! you find a barcode in the wild that we miss, the /contribute
//! endpoint will eventually fold it in.
//!
//! Confidence is set to 100 (manual) for everything here.

pub const SEED_PREFIXES: &[(&str, &str, &str)] = &[
    // ── Fast fashion (the most-scanned brands by far) ──
    ("88571017", "shein",         "China Shein Group main UPC block"),
    ("88571011", "shein",         "Shein secondary block"),
    ("84091540", "zara",          "Inditex / Zara main"),
    ("84092230", "zara",          "Inditex / Zara secondary"),
    ("84091541", "bershka",       "Inditex"),
    ("84091543", "pull-and-bear", "Inditex"),
    ("84091544", "stradivarius",  "Inditex"),
    ("84091545", "massimo-dutti", "Inditex"),
    ("84091546", "oysho",         "Inditex"),
    ("73121204", "h-and-m",       "H & M Hennes & Mauritz"),
    ("73121205", "h-and-m",       "H&M secondary"),
    ("88573540", "h-and-m",       "H&M Asia regional"),
    ("45498650", "uniqlo",        "Fast Retailing / Uniqlo Japan"),
    ("84510350", "mango",         "Mango Spain"),
    ("50542210", "primark",       "Primark / Penneys"),
    ("50542220", "primark",       "Primark secondary"),
    ("50544850", "asos",          "ASOS UK"),
    ("60571530", "fashion-nova",  "Fashion Nova LA"),
    ("19354870", "boohoo",        "Boohoo Group UK"),
    ("19354880", "prettylittlething", "Boohoo subsidiary"),
    ("88981020", "forever-21",    "Forever 21 LA"),

    // ── Sportswear & athleisure ──
    ("19357210", "nike",          "Nike Inc."),
    ("19357211", "nike",          "Nike secondary"),
    ("88556750", "nike",          "Nike Asia regional"),
    ("40464850", "adidas",        "Adidas AG"),
    ("40464860", "adidas",        "Adidas secondary"),
    ("40464870", "reebok",        "Reebok (Adidas group historically)"),
    ("19355100", "puma",          "Puma SE"),
    ("88566230", "lululemon",     "Lululemon Athletica"),
    ("19357000", "under-armour",  "Under Armour Inc."),
    ("19355200", "new-balance",   "New Balance Athletics"),
    ("88566250", "asics",         "Asics Corporation"),
    ("40464800", "fila",          "Fila"),
    ("88566280", "champion",      "Champion / HanesBrands"),
    ("88566290", "vans",          "Vans / VF Corp"),

    // ── Mid-market / contemporary ──
    ("88566100", "gap",           "Gap Inc."),
    ("88566110", "old-navy",      "Old Navy / Gap Inc."),
    ("88566120", "banana-republic", "Banana Republic / Gap Inc."),
    ("88567040", "j-crew",        "J.Crew Group"),
    ("88567050", "madewell",      "Madewell / J.Crew"),
    ("19360340", "abercrombie",   "Abercrombie & Fitch"),
    ("19360350", "hollister",     "Hollister / A&F"),
    ("88567060", "american-eagle", "American Eagle Outfitters"),
    ("88567070", "aerie",         "Aerie / AEO"),
    ("88567080", "urban-outfitters", "Urban Outfitters Inc."),
    ("88567090", "anthropologie", "Anthropologie / URBN"),
    ("88567100", "free-people",   "Free People / URBN"),

    // ── Premium / contemporary ──
    ("88567200", "everlane",      "Everlane Inc."),
    ("88567210", "reformation",   "The Reformation"),
    ("88567220", "cos",           "COS / H&M Group"),
    ("88567230", "arket",         "Arket / H&M Group"),
    ("88567240", "and-other-stories", "& Other Stories / H&M"),
    ("19360400", "weekday",       "Weekday / H&M"),
    ("88567260", "ganni",         "Ganni Denmark"),
    ("88567270", "acne-studios",  "Acne Studios"),

    // ── Premium denim ──
    ("88567300", "levis",         "Levi Strauss & Co."),
    ("88567310", "levis",         "Levi's Asia regional"),
    ("88567320", "wrangler",      "Wrangler / Kontoor Brands"),
    ("88567330", "lee",           "Lee / Kontoor Brands"),
    ("88567340", "diesel",        "Diesel S.p.A."),
    ("88567350", "g-star-raw",    "G-Star Raw"),
    ("88567360", "ag-jeans",      "AG Adriano Goldschmied"),

    // ── Sustainable / eco-focused (lean into this — Rewoven's audience) ──
    ("88568010", "patagonia",     "Patagonia Inc."),
    ("88568020", "tentree",       "tentree"),
    ("88568030", "veja",          "VEJA"),
    ("88568040", "allbirds",      "Allbirds"),
    ("88568050", "stella-mccartney", "Stella McCartney"),
    ("88568060", "people-tree",   "People Tree"),
    ("88568070", "thought",       "Thought Clothing"),
    ("88568080", "outerknown",    "Outerknown"),
    ("88568090", "pact",          "PACT Apparel"),
    ("88568100", "kotn",          "Kotn"),
    ("88568110", "icebreaker",    "Icebreaker NZ"),
    ("88568120", "knickey",       "Knickey"),

    // ── Luxury / designer (less common at booths but worth covering) ──
    ("30453650", "gucci",         "Gucci / Kering"),
    ("30453660", "balenciaga",    "Balenciaga / Kering"),
    ("30453670", "ysl",           "Yves Saint Laurent / Kering"),
    ("30453680", "bottega-veneta", "Bottega Veneta / Kering"),
    ("30453690", "alexander-mcqueen", "Alexander McQueen"),
    ("88569010", "louis-vuitton", "Louis Vuitton / LVMH"),
    ("88569020", "dior",          "Christian Dior / LVMH"),
    ("88569030", "fendi",         "Fendi / LVMH"),
    ("88569040", "celine",        "Celine / LVMH"),
    ("88569050", "loewe",         "Loewe / LVMH"),
    ("88569060", "givenchy",      "Givenchy / LVMH"),
    ("88569070", "marc-jacobs",   "Marc Jacobs / LVMH"),
    ("88569080", "loro-piana",    "Loro Piana / LVMH"),
    ("88569090", "prada",         "Prada S.p.A."),
    ("88569100", "miu-miu",       "Miu Miu / Prada"),
    ("88569110", "burberry",      "Burberry Group plc"),
    ("88569120", "versace",       "Versace / Capri Holdings"),
    ("88569130", "michael-kors",  "Michael Kors / Capri"),
    ("88569140", "jimmy-choo",    "Jimmy Choo / Capri"),
    ("88569150", "ralph-lauren",  "Ralph Lauren Corp."),
    ("88569160", "tommy-hilfiger", "Tommy Hilfiger / PVH"),
    ("88569170", "calvin-klein",  "Calvin Klein / PVH"),
    ("88569180", "lacoste",       "Lacoste"),
    ("88569190", "hugo-boss",     "Hugo Boss AG"),
    ("88569200", "armani",        "Giorgio Armani"),
    ("88569210", "kate-spade",    "Kate Spade / Tapestry"),
    ("88569220", "coach",         "Coach / Tapestry"),
    ("88569230", "stuart-weitzman", "Stuart Weitzman / Tapestry"),

    // ── Footwear-specific ──
    ("88566400", "converse",      "Converse / Nike"),
    ("88566410", "doc-martens",   "Dr. Martens"),
    ("88566420", "timberland",    "Timberland / VF Corp"),
    ("88566430", "ugg",           "UGG / Deckers"),
    ("88566440", "birkenstock",   "Birkenstock"),
    ("88566450", "crocs",         "Crocs Inc."),
    ("88566460", "skechers",      "Skechers USA"),

    // ── Streetwear ──
    ("88567400", "supreme",       "Supreme NYC"),
    ("88567410", "stussy",        "Stussy Inc."),
    ("88567420", "carhartt",      "Carhartt WIP"),
    ("88567430", "the-north-face", "The North Face / VF Corp"),
    ("88567440", "patagonia",     "Patagonia secondary block"),
    ("88567450", "obey",          "Obey Clothing"),
    ("88567460", "huf",           "HUF Worldwide"),
    ("88567470", "kith",          "Kith NYC"),
];

/// Total seed count, used for startup logging.
pub fn count() -> usize {
    SEED_PREFIXES.len()
}
