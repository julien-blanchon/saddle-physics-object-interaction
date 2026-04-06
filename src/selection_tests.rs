use super::*;
use crate::config::{AcquisitionConfig, TargetScoringConfig};

#[test]
fn direct_hit_and_priority_can_beat_distance() {
    let acquisition = AcquisitionConfig {
        max_distance: 8.0,
        sticky_target_bonus: 0.2,
        ..Default::default()
    };
    let scoring = TargetScoringConfig {
        distance_weight: 0.2,
        angle_weight: 0.3,
        priority_weight: 0.7,
        direct_hit_bonus: 0.4,
    };

    let near = score_candidate(
        CandidateScoreInput {
            distance: 1.5,
            alignment: 0.95,
            priority: 0.0,
            direct_hit: false,
            sticky: false,
        },
        &acquisition,
        &scoring,
    );
    let important = score_candidate(
        CandidateScoreInput {
            distance: 3.5,
            alignment: 0.98,
            priority: 1.4,
            direct_hit: true,
            sticky: false,
        },
        &acquisition,
        &scoring,
    );

    assert!(important > near);
}

#[test]
fn select_index_wraps_in_both_directions() {
    assert_eq!(select_index(4, Some(3), 1), Some(0));
    assert_eq!(select_index(4, Some(0), -1), Some(3));
    assert_eq!(select_index(4, None, 2), Some(2));
    assert_eq!(select_index(0, Some(0), 1), None);
}

#[test]
fn hold_distance_clamps_to_bounds() {
    assert_eq!(hold_distance_clamped(0.1, 0.75, 5.5), 0.75);
    assert_eq!(hold_distance_clamped(2.5, 0.75, 5.5), 2.5);
    assert_eq!(hold_distance_clamped(9.0, 0.75, 5.5), 5.5);
}

#[test]
fn hysteresis_keeps_previous_target_until_best_is_clearly_better() {
    assert!(should_keep_previous_target(0.92, 0.88, 0.05));
    assert!(!should_keep_previous_target(0.97, 0.88, 0.05));
    assert!(should_keep_previous_target(0.88, 0.88, 0.0));
}

#[test]
fn default_selection_scorer_uses_weighted_formula() {
    let context = SelectionScoringContext {
        interactor: Entity::from_raw_u32(1).unwrap(),
        origin: Vec3::ZERO,
        forward: -Vec3::Z,
        current_target: Some(Entity::from_raw_u32(7).unwrap()),
        acquisition: AcquisitionConfig {
            max_distance: 8.0,
            sticky_target_bonus: 0.2,
            ..Default::default()
        },
        scoring: TargetScoringConfig {
            distance_weight: 0.2,
            angle_weight: 0.3,
            priority_weight: 0.7,
            direct_hit_bonus: 0.4,
        },
    };
    let candidate = SelectionCandidate {
        entity: Entity::from_raw_u32(7).unwrap(),
        method: crate::CandidateMethod::DirectHit,
        distance: 3.5,
        alignment: 0.98,
        priority: 1.4,
        effective_mass: 8.0,
        hit_point: Some(Vec3::new(0.0, 0.5, -3.5)),
        target_position: Vec3::new(0.0, 0.5, -3.2),
        sticky: true,
    };

    let score = DefaultSelectionScorer.score(&context, &candidate);

    assert_eq!(
        score,
        score_candidate(
            CandidateScoreInput {
                distance: candidate.distance,
                alignment: candidate.alignment,
                priority: candidate.priority,
                direct_hit: true,
                sticky: true,
            },
            &context.acquisition,
            &context.scoring,
        )
    );
}

#[test]
fn selection_scorer_provider_supports_callbacks() {
    let provider = SelectionScorerProvider::from_callback(|_, candidate| {
        candidate.priority * 10.0 + candidate.effective_mass
    });
    let context = SelectionScoringContext {
        interactor: Entity::from_raw_u32(1).unwrap(),
        origin: Vec3::ZERO,
        forward: -Vec3::Z,
        current_target: None,
        acquisition: AcquisitionConfig::default(),
        scoring: TargetScoringConfig::default(),
    };
    let candidate = SelectionCandidate {
        entity: Entity::from_raw_u32(2).unwrap(),
        method: crate::CandidateMethod::Overlap,
        distance: 2.0,
        alignment: 0.8,
        priority: 1.2,
        effective_mass: 6.0,
        hit_point: Some(Vec3::new(0.0, 0.5, -2.0)),
        target_position: Vec3::new(0.0, 0.5, -2.0),
        sticky: false,
    };

    assert_eq!(provider.score(&context, &candidate), 18.0);
}
