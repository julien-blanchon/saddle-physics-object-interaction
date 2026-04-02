use crate::config::{AcquisitionConfig, TargetScoringConfig};

#[derive(Debug, Clone, Copy, PartialEq)]
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
