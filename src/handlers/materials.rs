use axum::{extract::Path, Json};
use std::sync::OnceLock;

use crate::error::AppError;
use crate::models::MaterialImpact;

static MATERIALS: OnceLock<Vec<MaterialImpact>> = OnceLock::new();

fn get_materials_data() -> &'static Vec<MaterialImpact> {
    MATERIALS.get_or_init(|| {
        let mut materials = vec![
            MaterialImpact { name: "Conventional Cotton".into(), slug: "conventional-cotton".into(), category: "Natural".into(), co2_kg_per_kg: 8.0, water_liters_per_kg: 10000.0, biodegradable: true, recyclable: true, sustainability_score: 35, description: "Most widely used natural fiber. Extremely water-intensive and often relies on pesticides.".into() },
            MaterialImpact { name: "Organic Cotton".into(), slug: "organic-cotton".into(), category: "Natural".into(), co2_kg_per_kg: 4.0, water_liters_per_kg: 7000.0, biodegradable: true, recyclable: true, sustainability_score: 72, description: "Grown without synthetic pesticides or fertilizers. Uses less water than conventional cotton.".into() },
            MaterialImpact { name: "Polyester".into(), slug: "polyester".into(), category: "Synthetic".into(), co2_kg_per_kg: 9.5, water_liters_per_kg: 60.0, biodegradable: false, recyclable: true, sustainability_score: 20, description: "Derived from petroleum. Low water use but high carbon footprint and sheds microplastics.".into() },
            MaterialImpact { name: "Recycled Polyester".into(), slug: "recycled-polyester".into(), category: "Recycled".into(), co2_kg_per_kg: 3.5, water_liters_per_kg: 40.0, biodegradable: false, recyclable: true, sustainability_score: 58, description: "Made from recycled PET bottles. 59% less energy than virgin polyester but still sheds microplastics.".into() },
            MaterialImpact { name: "Nylon".into(), slug: "nylon".into(), category: "Synthetic".into(), co2_kg_per_kg: 12.0, water_liters_per_kg: 100.0, biodegradable: false, recyclable: true, sustainability_score: 15, description: "Petroleum-based with very high CO2 emissions. Produces nitrous oxide, a potent greenhouse gas.".into() },
            MaterialImpact { name: "Recycled Nylon".into(), slug: "recycled-nylon".into(), category: "Recycled".into(), co2_kg_per_kg: 5.0, water_liters_per_kg: 60.0, biodegradable: false, recyclable: true, sustainability_score: 55, description: "Made from ocean waste and old fishing nets. Significantly lower impact than virgin nylon.".into() },
            MaterialImpact { name: "Linen".into(), slug: "linen".into(), category: "Natural".into(), co2_kg_per_kg: 1.5, water_liters_per_kg: 700.0, biodegradable: true, recyclable: true, sustainability_score: 85, description: "Made from flax plant. Very low water and pesticide needs. One of the most sustainable fabrics.".into() },
            MaterialImpact { name: "Hemp".into(), slug: "hemp".into(), category: "Natural".into(), co2_kg_per_kg: 1.2, water_liters_per_kg: 500.0, biodegradable: true, recyclable: true, sustainability_score: 90, description: "Requires minimal water, no pesticides, and improves soil health. Extremely sustainable choice.".into() },
            MaterialImpact { name: "Wool".into(), slug: "wool".into(), category: "Animal".into(), co2_kg_per_kg: 17.0, water_liters_per_kg: 15000.0, biodegradable: true, recyclable: true, sustainability_score: 40, description: "Natural and biodegradable but high water use and methane emissions from sheep farming.".into() },
            MaterialImpact { name: "Merino Wool".into(), slug: "merino-wool".into(), category: "Animal".into(), co2_kg_per_kg: 20.0, water_liters_per_kg: 17000.0, biodegradable: true, recyclable: true, sustainability_score: 38, description: "Premium wool with mulesing concerns. Durable and naturally temperature-regulating.".into() },
            MaterialImpact { name: "Silk".into(), slug: "silk".into(), category: "Animal".into(), co2_kg_per_kg: 15.0, water_liters_per_kg: 10000.0, biodegradable: true, recyclable: false, sustainability_score: 30, description: "Natural luxury fiber but involves killing silkworms. High water and energy consumption.".into() },
            MaterialImpact { name: "Peace Silk".into(), slug: "peace-silk".into(), category: "Animal".into(), co2_kg_per_kg: 16.0, water_liters_per_kg: 10500.0, biodegradable: true, recyclable: false, sustainability_score: 45, description: "Cruelty-free silk that allows moths to emerge before harvesting. Higher ethical standards.".into() },
            MaterialImpact { name: "Viscose/Rayon".into(), slug: "viscose-rayon".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 7.0, water_liters_per_kg: 3000.0, biodegradable: true, recyclable: false, sustainability_score: 30, description: "Made from wood pulp using chemical-intensive process. Often linked to deforestation.".into() },
            MaterialImpact { name: "Tencel/Lyocell".into(), slug: "tencel-lyocell".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 2.0, water_liters_per_kg: 1500.0, biodegradable: true, recyclable: true, sustainability_score: 82, description: "Made from sustainably sourced wood pulp in a closed-loop process. Very eco-friendly.".into() },
            MaterialImpact { name: "Modal".into(), slug: "modal".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 3.0, water_liters_per_kg: 2000.0, biodegradable: true, recyclable: true, sustainability_score: 70, description: "Made from beech tree pulp. More sustainable than viscose when sourced from managed forests.".into() },
            MaterialImpact { name: "Bamboo".into(), slug: "bamboo".into(), category: "Semi-Synthetic".into(), co2_kg_per_kg: 4.0, water_liters_per_kg: 800.0, biodegradable: true, recyclable: false, sustainability_score: 50, description: "Bamboo grows fast without pesticides but processing into fabric uses harsh chemicals.".into() },
            MaterialImpact { name: "Acrylic".into(), slug: "acrylic".into(), category: "Synthetic".into(), co2_kg_per_kg: 11.5, water_liters_per_kg: 200.0, biodegradable: false, recyclable: false, sustainability_score: 10, description: "Petroleum-based with high CO2 and toxic chemical use. Not recyclable or biodegradable.".into() },
            MaterialImpact { name: "Spandex/Elastane".into(), slug: "spandex-elastane".into(), category: "Synthetic".into(), co2_kg_per_kg: 10.0, water_liters_per_kg: 150.0, biodegradable: false, recyclable: false, sustainability_score: 12, description: "Petroleum-based stretch fiber. Cannot be recycled and makes blended fabrics harder to recycle.".into() },
            MaterialImpact { name: "Leather".into(), slug: "leather".into(), category: "Animal".into(), co2_kg_per_kg: 25.0, water_liters_per_kg: 17000.0, biodegradable: true, recyclable: false, sustainability_score: 18, description: "Extremely high environmental impact from cattle farming, tanning chemicals, and water use.".into() },
            MaterialImpact { name: "Vegan Leather (PU)".into(), slug: "vegan-leather-pu".into(), category: "Synthetic".into(), co2_kg_per_kg: 8.0, water_liters_per_kg: 200.0, biodegradable: false, recyclable: false, sustainability_score: 28, description: "Polyurethane-based alternative. Lower impact than leather but still petroleum-derived.".into() },
            MaterialImpact { name: "Piñatex".into(), slug: "pinatex".into(), category: "Innovative".into(), co2_kg_per_kg: 2.5, water_liters_per_kg: 300.0, biodegradable: true, recyclable: false, sustainability_score: 78, description: "Made from pineapple leaf fibers. Innovative, natural, and uses agricultural waste.".into() },
            MaterialImpact { name: "Mushroom Leather (Mylo)".into(), slug: "mushroom-leather".into(), category: "Innovative".into(), co2_kg_per_kg: 1.8, water_liters_per_kg: 200.0, biodegradable: true, recyclable: false, sustainability_score: 85, description: "Grown from mycelium in days. Very low environmental impact and fully biodegradable.".into() },
            MaterialImpact { name: "Recycled Cotton".into(), slug: "recycled-cotton".into(), category: "Recycled".into(), co2_kg_per_kg: 2.5, water_liters_per_kg: 1500.0, biodegradable: true, recyclable: true, sustainability_score: 75, description: "Made from pre- and post-consumer cotton waste. Significantly reduces water and energy use.".into() },
            MaterialImpact { name: "Cashmere".into(), slug: "cashmere".into(), category: "Animal".into(), co2_kg_per_kg: 28.0, water_liters_per_kg: 20000.0, biodegradable: true, recyclable: true, sustainability_score: 15, description: "Luxury fiber with severe environmental impact from goat overgrazing and desertification.".into() },
            MaterialImpact { name: "Down".into(), slug: "down".into(), category: "Animal".into(), co2_kg_per_kg: 22.0, water_liters_per_kg: 14000.0, biodegradable: true, recyclable: false, sustainability_score: 25, description: "Excellent insulator but serious animal welfare concerns with live-plucking and force-feeding.".into() },
            MaterialImpact { name: "Recycled Down".into(), slug: "recycled-down".into(), category: "Recycled".into(), co2_kg_per_kg: 3.0, water_liters_per_kg: 500.0, biodegradable: true, recyclable: false, sustainability_score: 70, description: "Reclaimed from old products. Same performance with dramatically lower environmental impact.".into() },
            MaterialImpact { name: "Econyl".into(), slug: "econyl".into(), category: "Recycled".into(), co2_kg_per_kg: 4.5, water_liters_per_kg: 50.0, biodegradable: false, recyclable: true, sustainability_score: 65, description: "Regenerated nylon from ocean waste, fabric scraps, and old carpets. Infinitely recyclable.".into() },
            MaterialImpact { name: "Cork Fabric".into(), slug: "cork-fabric".into(), category: "Innovative".into(), co2_kg_per_kg: 0.8, water_liters_per_kg: 100.0, biodegradable: true, recyclable: true, sustainability_score: 92, description: "Harvested from cork oak bark without killing the tree. Carbon-negative and biodegradable.".into() },
            MaterialImpact { name: "Seacell".into(), slug: "seacell".into(), category: "Innovative".into(), co2_kg_per_kg: 1.5, water_liters_per_kg: 300.0, biodegradable: true, recyclable: false, sustainability_score: 80, description: "Made from seaweed and wood cellulose. Naturally antibacterial with minimal processing.".into() },
            MaterialImpact { name: "Orange Fiber".into(), slug: "orange-fiber".into(), category: "Innovative".into(), co2_kg_per_kg: 2.0, water_liters_per_kg: 250.0, biodegradable: true, recyclable: false, sustainability_score: 82, description: "Made from citrus juice byproducts. Turns waste into luxury silk-like fabric.".into() },
        ];
        materials.sort_by(|a, b| b.sustainability_score.cmp(&a.sustainability_score));
        materials
    })
}

pub async fn get_materials() -> Json<Vec<MaterialImpact>> {
    Json(get_materials_data().clone())
}

pub async fn get_material(
    Path(slug): Path<String>,
) -> Result<Json<MaterialImpact>, AppError> {
    let slug_lower = slug.to_lowercase();
    match get_materials_data().iter().find(|m| m.slug == slug_lower) {
        Some(material) => Ok(Json(material.clone())),
        None => Err(AppError::NotFound(format!("Material '{}' not found", slug))),
    }
}
