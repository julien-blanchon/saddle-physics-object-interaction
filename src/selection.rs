use std::sync::Arc;

use bevy::prelude::*;

use crate::{
    components::CandidateMethod,
    config::{AcquisitionConfig, TargetScoringConfig},
};

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct SelectionScoringContext {
    pub interactor: Entity,
    pub origin: Vec3,
    pub forward: Vec3,
    pub current_target: Option<Entity>,
    pub acquisition: AcquisitionConfig,
    pub scoring: TargetScoringConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct SelectionCandidate {
    pub entity: Entity,
    pub method: CandidateMethod,
    pub distance: f32,
    pub alignment: f32,
    pub priority: f32,
    pub effective_mass: f32,
    pub hit_point: Option<Vec3>,
    pub target_position: Vec3,
    pub sticky: bool,
}

pub trait SelectionScorer: Send + Sync + 'static {
    fn score(&self, context: &SelectionScoringContext, candidate: &SelectionCandidate) -> f32;
}

#[derive(Resource, Clone)]
pub struct SelectionScorerProvider {
    scorer: Arc<dyn SelectionScorer>,
}

impl SelectionScorerProvider {
    pub fn from_scorer(scorer: impl SelectionScorer) -> Self {
        Self {
            scorer: Arc::new(scorer),
        }
    }

    pub fn from_callback(
        callback: impl Fn(&SelectionScoringContext, &SelectionCandidate) -> f32 + Send + Sync + 'static,
    ) -> Self {
        Self::from_scorer(CallbackSelectionScorer { callback })
    }

    pub fn score(&self, context: &SelectionScoringContext, candidate: &SelectionCandidate) -> f32 {
        self.scorer.score(context, candidate)
    }
}

struct CallbackSelectionScorer<F> {
    callback: F,
}

impl<F> SelectionScorer for CallbackSelectionScorer<F>
where
    F: Fn(&SelectionScoringContext, &SelectionCandidate) -> f32 + Send + Sync + 'static,
{
    fn score(&self, context: &SelectionScoringContext, candidate: &SelectionCandidate) -> f32 {
        (self.callback)(context, candidate)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultSelectionScorer;

impl SelectionScorer for DefaultSelectionScorer {
    fn score(&self, context: &SelectionScoringContext, candidate: &SelectionCandidate) -> f32 {
        score_candidate(
            CandidateScoreInput {
                distance: candidate.distance,
                alignment: candidate.alignment,
                priority: candidate.priority,
                direct_hit: candidate.method == CandidateMethod::DirectHit,
                sticky: candidate.sticky,
            },
            &context.acquisition,
            &context.scoring,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct CandidateScoreInput {
    pub distance: f32,
    pub alignment: f32,
    pub priority: f32,
    pub direct_hit: bool,
    pub sticky: bool,
}

pub fn cone_alignment_cutoff(acquisition: &AcquisitionConfig) -> f32 {
    acquisition.cone_half_angle_degrees.to_radians().cos()
}

pub fn score_candidate(
    input: CandidateScoreInput,
    acquisition: &AcquisitionConfig,
    scoring: &TargetScoringConfig,
) -> f32 {
    let distance_norm = 1.0 - (input.distance / acquisition.max_distance).clamp(0.0, 1.0);
    let angle_norm = input.alignment.clamp(0.0, 1.0);

    let mut score = distance_norm * scoring.distance_weight
        + angle_norm * scoring.angle_weight
        + input.priority.max(0.0) * scoring.priority_weight;

    if input.direct_hit {
        score += scoring.direct_hit_bonus;
    }
    if input.sticky {
        score += acquisition.sticky_target_bonus;
    }

    score
}

pub fn should_keep_previous_target(best_score: f32, current_score: f32, hysteresis: f32) -> bool {
    best_score <= current_score + hysteresis.max(0.0)
}

pub fn select_index(len: usize, current: Option<usize>, step: i32) -> Option<usize> {
    if len == 0 {
        return None;
    }

    let base = current.unwrap_or(0) as i32;
    let wrapped = (base + step).rem_euclid(len as i32) as usize;
    Some(wrapped)
}

pub fn hold_distance_clamped(distance: f32, min_distance: f32, max_distance: f32) -> f32 {
    distance.clamp(min_distance, max_distance)
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
