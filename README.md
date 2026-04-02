# Saddle Physics Object Interaction

Reusable 3D rigid-body interaction for Bevy + Avian3D: detect, acquire, pull, hold, inspect, drop, and throw dynamic bodies through a message-driven API.

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
    HoldDistance, InteractableBody, ObjectInteractionPlugin, ObjectInteractor, TryAcquireObject,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
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
app.add_plugins(ObjectInteractionPlugin::default());
```

Or inject your own schedules:

```rust
use saddle_physics_object_interaction::ObjectInteractionPlugin;

app.add_plugins(ObjectInteractionPlugin::new(
    OnEnter(MyState::Gameplay),
    OnExit(MyState::Gameplay),
    Update,
    avian3d::prelude::PhysicsSchedule,
));
```

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
| `PreferredHoldDistance` | Per-prop default hold distance override |
| `InteractionMassLimitOverride` | Per-prop effective mass override for selection/validation |
| `HoldPointOverride` / `HoldOrientationOverride` | Per-prop local anchor and rotation policy overrides |
| `InteractionCollisionPolicy` | Collision-layer behavior while a prop is held |
| `ThrowResponseOverride` | Per-prop throw impulse and spin scaling |
| `ObjectInteractionConfig` | Global acquisition, scoring, hold, and throw tuning |
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

## Examples

| Example | Run | What it demonstrates |
| --- | --- | --- |
| `basic` | `cargo run --example basic` | Message-driven acquire, hold, drop, throw, rotate, and candidate cycling |
| `gravity_gun` | `cargo run --example gravity_gun` | Stronger pull/throw tuning and higher mass budget |
| `inspect_rotate` | `cargo run --example inspect_rotate` | Close hold distance, aligned rotation, and inspection feel |
| `picking_integration` | `cargo run --example picking_integration` | Cursor-driven acquisition using Bevy mesh picking |
| `saddle-physics-object-interaction-lab` | `cargo run -p saddle-physics-object-interaction-lab` | Rich crate-local BRP/E2E verification app |

For batch verification, every example and the crate-local lab also support
`OBJECT_INTERACTION_EXIT_AFTER_SECONDS=<seconds>` so they can boot, render, and
shut down without an external shell timeout.

Run the example commands above from inside `shared/physics/saddle-physics-object-interaction/`.

## Crate-Local Lab

The richer showcase and verification app lives in:

[`examples/lab/README.md`](/Users/julienblanchon/Git/bevy_starter/shared/physics/saddle-physics-object-interaction/examples/lab/README.md)

The lab includes:

- a default crate pickup path
- an overweight rejection path
- an inspection-focused prop with different overrides
- an occlusion station for forced-release verification
- BRP-friendly runtime diagnostics
- targeted E2E scenarios for smoke, throw, rejection, occlusion, and inspect rotation

## E2E Verification

```bash
cargo run -p saddle-physics-object-interaction-lab --features e2e -- smoke_launch
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_smoke
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_throw
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_heavy_reject
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_obstruction_break
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_rotate_inspect
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
