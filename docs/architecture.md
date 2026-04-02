# Architecture

## Model

`saddle-physics-object-interaction` splits the problem into two roles:

- an `ObjectInteractor` entity that owns candidate scoring, command intake, and at most one active hold
- an `InteractableBody` entity that stays fully simulated as an Avian dynamic rigid body

The interactor never teleports the prop directly. Instead, the physics step computes a spring/damper force and an alignment torque toward a desired hold point in front of the interactor.

## Data Flow

### 1. Read commands

The runtime consumes message inputs:

- acquire
- explicit target selection
- drop
- throw
- hold-distance adjustment
- held-object rotation
- target cycling

These messages update per-interactor command state. Input helpers, AI controllers, replay tools, and E2E scenarios all drive the same surface.

### 2. Refresh candidates

Candidate refresh runs on the variable-rate update schedule:

- build an interactor-space origin from `Transform` + `InteractionAnchor`
- optionally perform a direct ray hit
- gather overlap candidates with a forgiving sphere
- reject bodies that are disabled, non-dynamic, already held, too far, too heavy, or blocked by line of sight
- score survivors by distance, view alignment, prop priority, sticky-target bonus, and direct-hit bonus
- keep the previous target when a newcomer only wins by less than `target_switch_hysteresis`
- write the sorted result into `InteractionCandidates` and the selected entry into `InteractionTarget`

The selected target is just data. Acquisition is still a separate phase.

### 3. Acquire

When an interactor has a pending acquire command:

- validate the chosen prop again
- resolve the local anchor point
  - center of mass
  - remembered hit point
  - per-prop custom local anchor
- choose the orientation policy
  - preserve world rotation
  - align to interactor
  - custom prop-local rotation
- optionally swap collision layers for the held phase
- claim the prop immediately inside the acquire pass so two interactors cannot successfully grab the same body in the same frame
- insert `Holding` on the interactor and `HeldBy` + `HeldRuntime` on the prop

This phase emits `ObjectAcquired`.

### 4. Maintain hold

Hold maintenance runs on the physics schedule so it stays aligned with Avian’s fixed-step world:

- compute the desired hold point from interactor transform, anchor offset, and `HoldDistance`
- derive the held body’s world-space anchor position from its physics state
- use a spring force toward the target point
- use an angular spring toward the desired orientation
- clamp both force and torque
- track instability and occlusion timers
- emit `HeldObjectBecameUnstable` when the grace window is exceeded

The prop remains dynamic the whole time. It can collide, swing, scrape, and be blocked by walls.

### 5. Release and throw

The physics step also resolves pending release requests:

- `Dropped` just clears held state and restores collision policy
- `Thrown` applies a linear impulse plus optional angular impulse before release
- forced releases use explicit reasons such as `DistanceExceeded`, `Occluded`, `Unstable`, or `TargetInvalid`

This phase emits `ObjectReleased` and, for throws, `ObjectThrown`.

## Schedule Reasoning

The plugin exposes two important execution lanes:

- `update_schedule`
  - `ReadCommands`
  - `RefreshCandidates`
  - `AcquireTargets`
  - `Presentation`
- `physics_schedule`
  - `MaintainHold`
  - `ReleaseAndThrow`

That split is intentional:

- command latency stays low because input and AI messages are consumed on the normal update loop
- hold forces remain stable because the solver runs inside the fixed-step physics schedule
- output/debug state can still be refreshed once per rendered frame

## Why Force-Based Movement Instead of Teleportation

Teleporting the held body to the hold point every frame looks acceptable in a toy demo, but it breaks the goals of a reusable physics-handle crate:

- collisions become unreliable or explosive
- heavy and light props feel identical
- stacked bodies do not react naturally
- throws inherit less believable motion
- networking and replay integration lose a clean physics story

The spring-damper model keeps the object simulated while still letting the consumer tune how “tight” or “loose” the handle feels.

## Collision Policy

Held objects often fight with the holder or camera rig. The crate keeps that concern generic through `InteractionCollisionPolicy`:

- `Preserve`
- `IgnoreInteractorLayer`
- `DisableAll`
- `CustomLayers(...)`

The runtime snapshots the previous collision-layer state and restores it on release.

## Per-Prop Overrides

Important feel changes belong on the prop, not in controller-specific glue:

- `PreferredHoldDistance`
- `InteractionMassLimitOverride`
- `HoldPointOverride`
- `HoldOrientationOverride`
- `InteractionCollisionPolicy`
- `ThrowResponseOverride`

This lets a crate, tool, orb, or saw blade each behave differently without changing the plugin API.
