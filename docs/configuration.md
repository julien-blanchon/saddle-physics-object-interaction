# Configuration

## `ObjectInteractionConfig`

`ObjectInteractionConfig` groups four tuning blocks:

- `acquisition`
- `scoring`
- `hold`
- `throw`

The defaults target a medium-weight gravity-handle feel for small props when you opt into the shipped `DefaultSelectionScorer` and `DefaultThrowProfile`.

## Acquisition

| Field | Type | Default | Valid Range | Effect |
| --- | --- | --- | --- | --- |
| `max_distance` | `f32` | `6.5` | `> 0.0` | Hard cap for candidate eligibility and direct pickup distance |
| `forgiving_radius` | `f32` | `1.1` | `>= 0.0` | Radius for overlap-based candidate gathering around the aim origin |
| `cone_half_angle_degrees` | `f32` | `20.0` | `0.0..90.0` | Narrows or widens the view cone used for candidate scoring/filtering |
| `max_target_mass` | `f32` | `45.0` | `> 0.0` | Default maximum mass unless the interactor overrides it |
| `require_line_of_sight` | `bool` | `true` | `true/false` | When true, blocked targets are rejected and held props can be released on occlusion |
| `sticky_target_bonus` | `f32` | `0.12` | `>= 0.0` | Small score bonus that keeps current focus stable when scores are close |
| `target_switch_hysteresis` | `f32` | `0.08` | `>= 0.0` | Minimum score margin a new candidate must beat the current target by before focus switches automatically |

Tune up:

- `max_distance` for telekinesis or gravity-gun reach
- `forgiving_radius` for more action-game leniency
- `sticky_target_bonus` when aim jitter causes annoying target flicker

Tune down:

- `cone_half_angle_degrees` for precision tools
- `max_target_mass` when you want props to stay lightweight and readable

## Scoring

`TargetScoringConfig` is consumed by the opt-in `DefaultSelectionScorer`. Custom `SelectionScorerProvider` resources can use these values, reinterpret them, or ignore them entirely.

| Field | Type | Default | Valid Range | Effect |
| --- | --- | --- | --- | --- |
| `distance_weight` | `f32` | `0.35` | `>= 0.0` | Raises the score of nearer props |
| `angle_weight` | `f32` | `0.45` | `>= 0.0` | Raises the score of props closer to the view center |
| `priority_weight` | `f32` | `0.2` | `>= 0.0` | Lets explicit prop priority beat raw proximity when needed |
| `direct_hit_bonus` | `f32` | `0.18` | `>= 0.0` | Biases exact ray hits over overlap-only candidates |

If a farther prop should still win because it is the important one, raise `priority_weight` or the prop’s own `InteractableBody::priority`.

If you do not install a `SelectionScorerProvider`, the crate still collects and filters candidates, but it keeps them in collected order instead of applying weighted reranking.

## Hold

| Field | Type | Default | Valid Range | Effect |
| --- | --- | --- | --- | --- |
| `min_distance` | `f32` | `0.75` | `> 0.0` | Minimum hold distance after clamping |
| `default_distance` | `f32` | `2.5` | `>= min_distance` | Starting hold distance for most interactors |
| `max_distance` | `f32` | `5.5` | `>= default_distance` | Maximum hold distance after clamping |
| `linear_stiffness` | `f32` | `150.0` | `> 0.0` | Pull strength toward the hold point |
| `linear_damping` | `f32` | `28.0` | `>= 0.0` | Counter-force that reduces oscillation and overshoot |
| `angular_stiffness` | `f32` | `64.0` | `> 0.0` | Rotational pull toward the desired hold orientation |
| `angular_damping` | `f32` | `12.0` | `>= 0.0` | Rotational damping that fights spin noise |
| `max_force` | `f32` | `2800.0` | `> 0.0` | Hard cap on linear hold force |
| `max_torque` | `f32` | `180.0` | `> 0.0` | Hard cap on angular hold torque |
| `break_distance` | `f32` | `4.2` | `> 0.0` | Immediate forced-release threshold if the held body falls too far behind the desired point |
| `instability_distance` | `f32` | `1.1` | `> 0.0` | Error distance that starts the instability timer |
| `instability_grace_seconds` | `f32` | `0.35` | `>= 0.0` | Time the handle tolerates instability before releasing |
| `occlusion_grace_seconds` | `f32` | `0.28` | `>= 0.0` | Time the handle tolerates line-of-sight blockage before releasing |
| `collision_policy` | `InteractionCollisionPolicy` | `IgnoreInteractorLayer` | enum | Default collision behavior while a prop is held |
| `orientation_mode` | `HoldOrientationMode` | `PreserveWorld` | enum | Global default orientation behavior used when an interactor or prop leaves its mode at `UseConfig` |
| `pull_to_hand` | `PullToHandConfig` | see below | struct | Shapes how a newly acquired prop eases into the steady hold distance |
| `surface_placement` | `SurfacePlacementConfig` | see below | struct | Controls tracing and alignment when placement mode is enabled |

Tune up:

- `linear_stiffness` and `max_force` for a firmer, more “gun-like” handle
- `angular_stiffness` for inspection or carry modes that should face the actor cleanly
- `occlusion_grace_seconds` when brief wall clips should not instantly drop the prop

Tune down:

- `linear_stiffness` if heavy props oscillate
- `break_distance` if you want the handle to fail sooner and avoid long rubber-band stretches
- `angular_stiffness` if props spin too aggressively when the actor turns quickly

`HoldDistance` is the mutable per-interactor runtime value. New interactors that rely on the required-component default are seeded from `hold.default_distance`; consumers can still provide an explicit `HoldDistance` when spawning an interactor.

### `PullToHandConfig`

| Field | Type | Default | Valid Range | Effect |
| --- | --- | --- | --- | --- |
| `enabled` | `bool` | `true` | `true/false` | Enables the eased pickup path instead of snapping directly to the steady hold distance |
| `duration_seconds` | `f32` | `0.22` | `>= 0.0` | Duration of the pickup easing window |
| `arc_height` | `f32` | `0.28` | `>= 0.0` | Peak upward lift applied halfway through the pickup path |
| `min_start_distance` | `f32` | `0.4` | `>= 0.0` | Minimum starting offset used when the prop begins very close to the actor |

### `SurfacePlacementConfig`

| Field | Type | Default | Valid Range | Effect |
| --- | --- | --- | --- | --- |
| `max_distance` | `f32` | `5.5` | `> 0.0` | How far placement mode traces ahead of the interactor |
| `probe_radius` | `f32` | `0.18` | `>= 0.0` | Radius of the shape cast used to find walls, shelves, and other placement surfaces |
| `surface_offset` | `f32` | `0.05` | `>= 0.0` | Stand-off distance from the hit surface so the held anchor does not clip into geometry |
| `align_to_surface` | `bool` | `true` | `true/false` | Rotates the held prop to the traced surface frame instead of preserving actor-facing rotation |

`SurfacePlacementMode` is the per-interactor runtime toggle that turns this placement behavior on and off.

## Throw

`ThrowConfig` is consumed by the opt-in `DefaultThrowProfile`. Custom `ThrowProfileProvider` resources can map `ThrowHeldObject` intent to any linear/angular impulse pair they need.

| Field | Type | Default | Valid Range | Effect |
| --- | --- | --- | --- | --- |
| `impulse` | `f32` | `16.0` | `> 0.0` | Base forward throw impulse used by `DefaultThrowProfile` |
| `angular_impulse` | `f32` | `2.4` | `>= 0.0` | Base spin used by `DefaultThrowProfile` |
| `upward_bias` | `f32` | `0.08` | `>= 0.0` | Extra lift used by `DefaultThrowProfile` |
| `inherit_actor_velocity` | `bool` | `true` | `true/false` | Adds actor velocity when the active throw profile chooses to inherit it |

Per-prop `ThrowResponseOverride` supplies scalar hints to the active throw profile without requiring global retuning.

If you do not install a `ThrowProfileProvider`, `ThrowHeldObject` still releases the held prop, but no extra linear/angular impulse is added.

## Per-Prop Overrides

| Type | Purpose |
| --- | --- |
| `PreferredHoldDistance` | Sets a different initial hold distance for that prop |
| `InteractionMassLimitOverride` | Replaces the effective mass used by target validation |
| `HoldPointOverride` | Supplies a prop-local anchor point and optional rotation |
| `HoldOrientationOverride` | Overrides the default orientation mode |
| `InteractionCollisionPolicy` | Chooses a different held collision policy for that prop |
| `ThrowResponseOverride` | Scales the active throw profile's base impulse/spin and can override velocity inheritance |
