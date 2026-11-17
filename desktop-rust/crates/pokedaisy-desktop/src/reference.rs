use serde::{Deserialize, Serialize};

const GEN3_TYPE_CHART_JSON: &str = include_str!("../../../assets/reference/type-chart-gen3.json");

#[derive(Debug, Serialize, Deserialize)]
pub struct TypeChartDataset {
    pub schema_version: u32,
    pub dataset_id: String,
    pub generation: u8,
    pub source_sha256: String,
    pub license: String,
    pub default_percent: u16,
    pub types: Vec<TypeEntry>,
    pub matchups: Vec<TypeMatchup>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TypeEntry {
    pub id: u8,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TypeMatchup {
    pub attack: u8,
    pub defense: u8,
    pub percent: u16,
}

pub fn gen3_type_chart() -> Result<TypeChartDataset, String> {
    let dataset: TypeChartDataset =
        serde_json::from_str(GEN3_TYPE_CHART_JSON).map_err(|error| error.to_string())?;
    if dataset.schema_version != 1
        || dataset.dataset_id != "pokemon-gen3-type-chart"
        || dataset.generation != 3
        || dataset.source_sha256.len() != 64
        || dataset.license != "GPL-3.0-only"
    {
        return Err("La tabla de tipos de Gen 3 no supera la validación del manifiesto".into());
    }
    Ok(dataset)
}

impl TypeChartDataset {
    pub fn multiplier(&self, attack: u8, defense: u8) -> u16 {
        self.matchups
            .iter()
            .find(|pair| pair.attack == attack && pair.defense == defense)
            .map_or(self.default_percent, |pair| pair.percent)
    }

    pub fn combined_multiplier(&self, attack: u8, defenses: &[u8]) -> u16 {
        let mut result = 100u32;
        let mut previous = None;
        for defense in defenses.iter().copied() {
            if previous == Some(defense) {
                continue;
            }
            result = result * self.multiplier(attack, defense) as u32 / 100;
            previous = Some(defense);
        }
        result.min(u16::MAX as u32) as u16
    }
}

#[derive(Debug, Serialize)]
pub struct TypeMatchupResult {
    pub dataset_id: String,
    pub attack_type: String,
    pub defense_types: Vec<String>,
    pub multiplier_percent: u16,
}

pub fn resolve_matchup(
    dataset: &TypeChartDataset,
    attack: u8,
    defenses: &[u8],
) -> Result<TypeMatchupResult, String> {
    if defenses.is_empty() || defenses.len() > 2 {
        return Err("Selecciona uno o dos tipos defensores".into());
    }
    let attack_type = dataset
        .types
        .iter()
        .find(|entry| entry.id == attack)
        .map(|entry| entry.name.clone())
        .ok_or("El tipo atacante no pertenece a este dataset")?;
    let defense_types = defenses
        .iter()
        .map(|id| {
            dataset
                .types
                .iter()
                .find(|entry| entry.id == *id)
                .map(|entry| entry.name.clone())
                .ok_or_else(|| "Un tipo defensor no pertenece a este dataset".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TypeMatchupResult {
        dataset_id: dataset.dataset_id.clone(),
        attack_type,
        defense_types,
        multiplier_percent: dataset.combined_multiplier(attack, defenses),
    })
}

#[cfg(test)]
mod tests {
    use super::{gen3_type_chart, resolve_matchup};

    #[test]
    fn bundled_type_chart_matches_kotlin_reference_facts() {
        let chart = gen3_type_chart().unwrap();
        assert_eq!(chart.dataset_id, "pokemon-gen3-type-chart");
        assert_eq!(chart.multiplier(10, 12), 200); // Fire against Grass.
        assert_eq!(chart.multiplier(7, 0), 0); // Ghost against Normal.
        assert_eq!(chart.combined_multiplier(10, &[12, 8]), 400); // Grass / Steel.
    }

    #[test]
    fn chart_does_not_invent_unlisted_pairs() {
        let chart = gen3_type_chart().unwrap();
        assert_eq!(chart.multiplier(0, 0), 100);
    }

    #[test]
    fn matchup_result_rejects_ids_outside_this_dataset() {
        let chart = gen3_type_chart().unwrap();
        assert!(resolve_matchup(&chart, 10, &[99]).is_err());
    }
}
