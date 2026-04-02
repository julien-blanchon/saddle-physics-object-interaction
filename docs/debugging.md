# Debugging

## Recommended Runtime Surface

The crate exposes two useful resources for inspection:

- `ObjectInteractionDiagnostics`
- `ObjectInteractionDebugSettings`

The crate-local lab also exposes `common::DemoDiagnostics`, which gives higher-level names and counts for E2E and BRP workflows.

## Common Symptoms

### Nothing gets acquired

Check:

- the interactor has `ObjectInteractor`
- the prop has `InteractableBody`
- the prop is `RigidBody::Dynamic`
- the prop has a collider and valid mass properties
- the prop is inside `max_distance`
- the interactor is actually facing the prop
- `require_line_of_sight` is not rejecting the target
- the prop is not already held

### The wrong prop wins

Check:

- `InteractableBody::priority`
- `distance_weight`, `angle_weight`, and `priority_weight`
- `direct_hit_bonus`
- `sticky_target_bonus`
- whether the candidate comes from overlap-only or a direct ray hit

### Held props jitter

Common causes:

- missing `TransformInterpolation`
- `linear_stiffness` too high for the prop mass
- `linear_damping` too low
- `max_force` too low, so the prop never catches up
- collision with the holder or camera rig
- the physics schedule is not actually the fixed-step schedule you intended

### Throws feel weak

Check:

- `throw.impulse`
- `ThrowResponseOverride::impulse_scale`
- actor forward direction
- whether actor velocity inheritance is enabled
- whether the prop is still colliding immediately after release

## BRP Workflow

Start the crate-local lab:

```bash
cargo run -p saddle-physics-object-interaction-lab
```

Inspect from another terminal:

```bash
uv run --project .codex/skills/bevy-brp/script brp ping
uv run --project .codex/skills/bevy-brp/script brp world query bevy_ecs::name::Name
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_physics_object_interaction::debug::ObjectInteractionDiagnostics
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_physics_object_interaction_lab::common::DemoDiagnostics
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/object_interaction_lab.png
```

If you are unsure about the exact reflected type path, run `uv run --project .codex/skills/bevy-brp/script brp resource list` first and copy the path from the live registry.

Useful questions to answer with BRP:

- Which entity is currently targeted?
- Which entity is held?
- What was the last failure reason?
- What was the last release reason?
- How many acquisitions/releases/throws have occurred?
- Is the hold distance changing as expected?

## Recommended Checks During Tuning

1. Confirm `InteractionTarget` changes when you aim or cycle.
2. Confirm `Holding` and `HeldBy` are both present during the hold and both removed after release.
3. Confirm `ObjectInteractionDiagnostics.last_failure` or `last_release` matches the behavior you saw.
4. Confirm the prop has `TransformInterpolation` if the motion looks choppy.
5. Confirm collision layers are restored after release if you use a temporary held-phase policy.

## Performance Hotspots

Watch for:

- very large forgiving overlap radii with many nearby props
- too many line-of-sight raycasts in dense scenes
- debug gizmos left enabled in content-heavy maps
- very high stiffness with very large mass ratios

The crate-local lab includes a heavier spool and an occlusion wall specifically to make those failure modes easy to reproduce.
