# Saddle Physics Object Interaction

Reusable 3D rigid-body interaction for Bevy + Avian3D: detect, acquire, pull, hold, inspect, place against surfaces, drop, and throw dynamic bodies through a message-driven API.

The crate stays generic. It owns candidate selection, hold stabilization, release semantics, diagnostics, and output messages. Consumer code owns controller-specific input, UI, VFX, audio, quest logic, and any actor locomotion stack.

## Dependency Expectations

- `bevy = "0.18"`
- `avian3d = "0.6"`
- the direct `avian3d` dependency is intentional because the runtime needs Avian spatial queries, collision layers, rigid-body forces, and the fixed-step `PhysicsSchedule`
- dynamic props should use `RigidBody::Dynamic` plus a collider and mass properties
- held props should usually use `TransformInterpolation` to avoid visible jitter during fixed-step motion

## Quick Start

```toml
[dependencies]
saddle-physics-object-interaction = { git = "https://github.com/julien-blanchon/saddle-physics-object-interaction" }
avian3d = "0.6"
bevy = "0.18"
```

```rust
use avian3d::prelude::*;
use bevy::prelude::*;
use saddle_physics_object_interaction::{
    DefaultSelectionScorer, DefaultThrowProfile, HoldDistance, InteractableBody,
    ObjectInteractionPlugin, ObjectInteractor, SelectionScorerProvider, ThrowProfileProvider,
    TryAcquireObject,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
        .insert_resource(SelectionScorerProvider::from_scorer(
            DefaultSelectionScorer,
        ))
        .insert_resource(ThrowProfileProvider::from_profile(DefaultThrowProfile))
        .add_plugins(ObjectInteractionPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, trigger_grab)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Interactor"),
        ObjectInteractor::default(),
        HoldDistance(2.5),
        Transform::from_xyz(0.0, 1.4, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::IDENTITY,
    ));

    commands.spawn((
        Name::new("Crate"),
        InteractableBody::default(),
        RigidBody::Dynamic,
        Collider::cuboid(0.45, 0.45, 0.45),
        Mass(6.0),
        TransformInterpolation::default(),
        Transform::from_xyz(0.0, 0.8, 0.0),
        GlobalTransform::IDENTITY,
    ));
}

fn trigger_grab(
    interactor: Single<Entity, With<ObjectInteractor>>,
    mut acquire: MessageWriter<TryAcquireObject>,
) {
    acquire.write(TryAcquireObject {
        interactor: *interactor,
    });
}
```

## Plugin Usage

Use the default always-on wiring:

```rust
app.insert_resource(SelectionScorerProvider::from_scorer(
    DefaultSelectionScorer,
));
app.insert_resource(ThrowProfileProvider::from_profile(DefaultThrowProfile));
app.add_plugins(ObjectInteractionPlugin::default());
```

Or inject your own schedules:

```rust
use saddle_physics_object_interaction::{
    DefaultSelectionScorer, DefaultThrowProfile, ObjectInteractionPlugin,
    SelectionScorerProvider, ThrowProfileProvider,
};

app.insert_resource(SelectionScorerProvider::from_scorer(
    DefaultSelectionScorer,
));
app.insert_resource(ThrowProfileProvider::from_profile(DefaultThrowProfile));
app.add_plugins(ObjectInteractionPlugin::new(
    OnEnter(MyState::Gameplay),
    OnExit(MyState::Gameplay),
    Update,
    avian3d::prelude::PhysicsSchedule,
));
```

## Selection And Throw Policies

Candidate collection is built into the crate. Ranking and throw shaping are now explicit, optional policies:

- `SelectionScorerProvider` reranks validated candidates after collection
- `ThrowProfileProvider` maps `ThrowHeldObject` intent into arbitrary linear and angular impulses

If you do not install a scorer, the crate keeps candidates in collected order (`DirectHit` first, then overlap hits by collection order/distance). If you do not install a throw profile, `ThrowHeldObject` releases the held prop without adding extra impulse.

`DefaultSelectionScorer` and `DefaultThrowProfile` keep the previous weighted target ranking and forward-plus-up lobbed throw behavior, and all shipped examples opt into them explicitly.

## Public API

| Type | Purpose |
| --- | --- |
| `ObjectInteractionPlugin` | Registers the runtime with injectable activate/deactivate/update/physics schedules |
| `ObjectInteractionSystems` | Public ordering hooks for commands, candidate refresh, acquisition, hold maintenance, release, and presentation |
| `ObjectInteractor` | Actor-side configuration for candidate filtering, acquisition mode, line-of-sight checks, and orientation behavior |
| `InteractableBody` | Opt-in marker and priority/anchor metadata for props that can be manipulated |
| `InteractionAnchor` / `HoldDistance` | Actor-side hold-point offset and current hold distance |
| `InteractionTarget` / `InteractionCandidates` | Current selected target and sorted candidate list |
| `ObjectInteractionState` / `Holding` / `HeldBy` | Runtime state for idle, targeting, and active holds |
| `SurfacePlacementMode` | Per-interactor toggle that redirects the hold target onto a traced surface |
| `PreferredHoldDistance` | Per-prop default hold distance override |
| `InteractionMassLimitOverride` | Per-prop effective mass override for selection/validation |
| `HoldPointOverride` / `HoldOrientationOverride` | Per-prop local anchor and rotation policy overrides |
| `InteractionCollisionPolicy` | Collision-layer behavior while a prop is held |
| `SelectionScorerProvider` / `SelectionScorer` | Optional post-collection ranking hook for target selection |
| `ThrowProfileProvider` / `ThrowProfile` | Optional throw-intent mapper for custom linear/angular impulses |
| `SelectionCandidate` / `SelectionScoringContext` | Data passed to custom selection scorers |
| `ThrowImpulse` / `ThrowProfileContext` | Data passed to custom throw profiles |
| `DefaultSelectionScorer` / `DefaultThrowProfile` | Opt-in helpers that keep the previous weighted ranking and lobbed throw feel |
| `ThrowResponseOverride` | Per-prop throw scaling and velocity-inheritance hints consumed by throw profiles |
| `PullToHandConfig` / `SurfacePlacementConfig` | Global hold sub-configs for pickup easing and wall/shelf placement |
| `ObjectInteractionConfig` | Global acquisition, hold, and default scorer/profile tuning |
| `ObjectInteractionDiagnostics` / `ObjectInteractionDebugSettings` | Runtime counters and optional gizmo controls |

## Required Components

Interactor entities typically need:

- `ObjectInteractor`
- `Transform` / `GlobalTransform`
- optional `HoldDistance`
- optional `InteractionAnchor`
- optional `CollisionLayers` if you want `IgnoreInteractorLayer` to matter against a physical holder

Interactable props typically need:

- `InteractableBody`
- `RigidBody::Dynamic`
- `Collider`
- mass data such as `Mass`, `ColliderDensity`, or `MassPropertiesBundle`
- `TransformInterpolation` for smooth presentation

## Message Surface

Input messages:

- `TryAcquireObject`
- `SetInteractionTarget`
- `ReleaseHeldObject`
- `ThrowHeldObject`
- `AdjustHoldDistance`
- `RotateHeldObject`
- `SetSurfacePlacementMode`
- `CycleInteractionTarget`

Output messages:

- `ObjectAcquired`
- `ObjectReleased`
- `ObjectThrown`
- `ObjectInteractionFailed`
- `HeldObjectBecameUnstable`

## Fixed-Step and Interpolation

The crate is split on purpose:

- command processing and target refresh run on the variable-rate update schedule
- hold stabilization and release/throw resolution run on the physics schedule

That keeps the handle stable under fixed-step physics while still letting player input, AI, replay tools, and tests drive the runtime through messages. For dynamic props, add `TransformInterpolation` so the held object looks smooth between fixed ticks.

New holds now support two higher-level presentation layers without giving up the fixed-step spring model:

- `pull_to_hand` eases a newly acquired prop from its pickup point into the steady-state hold distance with a configurable arc
- `SurfacePlacementMode` traces ahead of the interactor and snaps the held anchor point onto shelves, walls, and panels for puzzle-style placement

## Examples

All examples feature FPS-style movement (WASD + mouse look) so you can walk around the scene and interact with objects naturally. Mouse controls: LMB grab/throw, RMB release, scroll wheel adjusts hold distance.

| Example | Run | What it demonstrates |
| --- | --- | --- |
| `basic` | `cargo run -p saddle-physics-object-interaction-example-basic` | Physics playground with FPS movement, mouse interaction, varied props in a room |
| `gravity_gun` | `cargo run -p saddle-physics-object-interaction-example-gravity_gun` | Stronger pull/throw tuning (28 impulse), 120 kg mass limit, heavy objects |
| `gravity_gun_combo` | `cargo run -p saddle-physics-object-interaction-example-gravity_gun_combo` | Cross-crate gravity-gun puzzle room using destruction and transform interpolation |
| `inspect_rotate` | `cargo run -p saddle-physics-object-interaction-example-inspect_rotate` | Close hold distance, aligned rotation, and inspection feel |
| `picking_integration` | `cargo run -p saddle-physics-object-interaction-example-picking_integration` | Click-to-acquire using Bevy mesh picking, with FPS mode toggle (RMB) |
| `surface_placement` | `cargo run -p saddle-physics-object-interaction-example-surface-placement` | Wall/shelf placement flow with pull-to-hand easing (G to toggle placement) |
| `saddle-physics-object-interaction-lab` | `cargo run -p saddle-physics-object-interaction-lab` | Rich crate-local BRP/E2E verification app with station teleport (1-5) |

### Controls (all examples)

| Input | Action |
| --- | --- |
| WASD | Move |
| Mouse | Look |
| Shift | Sprint |
| LMB | Grab (idle) / Throw (holding) |
| RMB | Release |
| Scroll | Adjust hold distance |
| E | Grab (keyboard) |
| R | Release (keyboard) |
| F | Throw (keyboard) |
| Q/C | Rotate held object |
| Z/X | Adjust hold distance |
| Tab | Cycle targets |
| Esc | Release cursor |

All example workspaces include `saddle-pane` so the hold, pull-to-hand, throw, placement, and combo-room parameters can be edited live while the demo runs.

All shipped examples now opt into `DefaultSelectionScorer` and `DefaultThrowProfile` explicitly so the old ranking and throw feel stays documented rather than hidden inside the runtime.

The combo example is intentionally wired the same way downstream games would do it: the local object-interaction crate stays on the workspace path, while `saddle-physics-destruction` and `saddle-physics-transform-interpolation` are pulled in through Git dependencies to prove the public APIs compose cleanly across repos.

For batch verification, every example and the crate-local lab also support
`OBJECT_INTERACTION_EXIT_AFTER_SECONDS=<seconds>` so they can boot, render, and
shut down without an external shell timeout.

Run the example commands above from inside `shared/physics/saddle-physics-object-interaction/`.

## Crate-Local Lab

The richer showcase and verification app lives in:

[`examples/lab/README.md`](examples/lab/README.md)

The lab includes:

- a default crate pickup path
- an overweight rejection path
- an inspection-focused prop with different overrides
- an occlusion station for forced-release verification
- a placement wall for surface-snapped carry/puzzle setups
- BRP-friendly runtime diagnostics
- targeted E2E scenarios for smoke, throw, rejection, occlusion, inspect rotation, and surface placement

## E2E Verification

```bash
cargo run -p saddle-physics-object-interaction-lab --features e2e -- smoke_launch
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_smoke
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_throw
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_heavy_reject
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_obstruction_break
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_rotate_inspect
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_surface_placement
```

## BRP Inspection

Run the crate-local lab in one terminal:

```bash
cargo run -p saddle-physics-object-interaction-lab
```

Inspect it from another:

```bash
uv run --project .codex/skills/bevy-brp/script brp ping
uv run --project .codex/skills/bevy-brp/script brp world query bevy_ecs::name::Name
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_physics_object_interaction::debug::ObjectInteractionDiagnostics
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_physics_object_interaction_lab::common::DemoDiagnostics
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/object_interaction_lab.png
uv run --project .codex/skills/bevy-brp/script brp extras shutdown
```

## More Detail

- [Architecture](docs/architecture.md)
- [Configuration](docs/configuration.md)
- [Debugging](docs/debugging.md)
