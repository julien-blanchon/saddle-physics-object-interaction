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
