use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::{Datelike, Utc};

use crate::types::Observation;

#[derive(Debug, Clone)]
pub struct DiversityScorer {
    taxon_counts: HashMap<u64, u32>,
    observer_counts: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
pub struct ScoredObservation {
    pub observation: Observation,
    pub score: f64,
}

impl DiversityScorer {
    pub fn new(taxon_counts: HashMap<u64, u32>, observer_counts: HashMap<String, u32>) -> Self {
        Self {
            taxon_counts,
            observer_counts,
        }
    }

    pub fn score(&self, observation: &Observation) -> f64 {
        let photo_quality_score = self.photo_quality_score(observation);
        let taxon_diversity_bonus = self.taxon_diversity_bonus(observation);
        let observer_diversity_bonus = self.observer_diversity_bonus(observation);
        let recency_bonus = self.recency_bonus(observation);
        let duplicate_penalty = self.duplicate_penalty(observation);
        let same_observer_penalty = self.same_observer_penalty(observation);

        // Keep quality as the base signal, then use diversity and recency as tie-breakers.
        // Strong penalties prevent over-selecting already saturated taxa or observers.
        photo_quality_score
            + taxon_diversity_bonus
            + observer_diversity_bonus
            + recency_bonus
            - duplicate_penalty
            - same_observer_penalty
    }

    pub fn record_selection(&mut self, observation: &Observation) {
        if let Some(taxon_id) = observation.taxon.as_ref().map(|taxon| taxon.id) {
            let next = self.taxon_counts.get(&taxon_id).copied().unwrap_or(0) + 1;
            self.taxon_counts.insert(taxon_id, next);
        }

        if let Some(observer_login) = observation.user.as_ref().map(|user| user.login.as_str()) {
            let next = self.observer_counts.get(observer_login).copied().unwrap_or(0) + 1;
            self.observer_counts.insert(observer_login.to_owned(), next);
        }
    }

    fn photo_quality_score(&self, observation: &Observation) -> f64 {
        let Some(photo) = observation.photos.first() else {
            return 2.5;
        };

        let Some(dimensions) = &photo.original_dimensions else {
            return 2.5;
        };

        let area = dimensions.width as f64 * dimensions.height as f64;
        let megapixels = area / 1_000_000.0;
        megapixels.clamp(0.5, 5.0)
    }

    fn taxon_diversity_bonus(&self, observation: &Observation) -> f64 {
        let Some(taxon_id) = observation.taxon.as_ref().map(|taxon| taxon.id) else {
            return 4.0;
        };

        let count = self.taxon_counts.get(&taxon_id).copied().unwrap_or(0);
        match count {
            0 => 10.0,
            1 => 5.0,
            _ => 2.0 / count as f64,
        }
    }

    fn observer_diversity_bonus(&self, observation: &Observation) -> f64 {
        let Some(observer_login) = observation.user.as_ref().map(|user| user.login.as_str()) else {
            return 2.0;
        };

        let count = self.observer_counts.get(observer_login).copied().unwrap_or(0);
        match count {
            0 => 6.0,
            1 => 3.0,
            _ => 2.0 / count as f64,
        }
    }

    fn recency_bonus(&self, observation: &Observation) -> f64 {
        let Some(year) = observation.observed_on_details.as_ref().and_then(|d| d.year) else {
            return 0.5;
        };

        let current_year = Utc::now().year();
        let obs_year = year as i32;

        if obs_year >= current_year {
            2.0
        } else if obs_year == current_year - 1 {
            1.5
        } else if obs_year == current_year - 2 {
            1.0
        } else {
            0.5
        }
    }

    fn duplicate_penalty(&self, observation: &Observation) -> f64 {
        let Some(taxon_id) = observation.taxon.as_ref().map(|taxon| taxon.id) else {
            return 0.0;
        };

        let count = self.taxon_counts.get(&taxon_id).copied().unwrap_or(0);
        if count >= 5 {
            5.0
        } else {
            0.0
        }
    }

    fn same_observer_penalty(&self, observation: &Observation) -> f64 {
        let Some(observer_login) = observation.user.as_ref().map(|user| user.login.as_str()) else {
            return 0.0;
        };

        let count = self.observer_counts.get(observer_login).copied().unwrap_or(0);
        if count >= 5 {
            3.0
        } else {
            0.0
        }
    }
}

pub fn select_top_n(
    observations: Vec<Observation>,
    scorer: &mut DiversityScorer,
    n: usize,
) -> Vec<ScoredObservation> {
    let mut scored: Vec<ScoredObservation> = observations
        .into_iter()
        .map(|observation| {
            let score = scorer.score(&observation);
            ScoredObservation { observation, score }
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    let mut selected = scored;
    selected.truncate(n);

    for item in &selected {
        scorer.record_selection(&item.observation);
    }

    selected
}
