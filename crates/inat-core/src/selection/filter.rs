use crate::config::Settings;
use crate::types::{Observation, annotation_terms, annotation_values};

#[derive(Debug, Clone, Copy)]
pub struct AnnotationFilter {
    pub exclude_dead: bool,
    pub exclude_non_organism: bool,
}

impl AnnotationFilter {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            exclude_dead: settings.exclude_dead,
            exclude_non_organism: settings.exclude_non_organism,
        }
    }

    pub fn should_include(&self, observation: &Observation) -> bool {
        if self.exclude_dead {
            let has_dead_annotation = observation.annotations.iter().any(|annotation| {
                annotation.controlled_attribute_id == annotation_terms::ALIVE_OR_DEAD
                    && annotation.controlled_value_id == annotation_values::DEAD
            });

            if has_dead_annotation {
                return false;
            }
        }

        if self.exclude_non_organism {
            let has_non_organism_annotation = observation.annotations.iter().any(|annotation| {
                annotation.controlled_attribute_id == annotation_terms::EVIDENCE_OF_ORGANISM
                    && annotation.controlled_value_id != annotation_values::ORGANISM
            });

            if has_non_organism_annotation {
                return false;
            }
        }

        true
    }
}

pub fn filter_observations(
    observations: Vec<Observation>,
    filter: &AnnotationFilter,
) -> Vec<Observation> {
    observations
        .into_iter()
        .filter(|observation| filter.should_include(observation))
        .collect()
}
