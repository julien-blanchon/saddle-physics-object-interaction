# saddle-physics-object-interaction-lab

Crate-local showcase and verification app for `saddle-physics-object-interaction`.

## Purpose

- exercise the same message-driven runtime used by the standalone examples
- provide a BRP-friendly scene with named props and live diagnostics
- host targeted `bevy_e2e` scenarios for acquisition, release, throw, heavy rejection, occlusion break, and inspect rotation

## Run

```bash
cargo run -p saddle-physics-object-interaction-lab
```

For non-interactive verification:

```bash
OBJECT_INTERACTION_EXIT_AFTER_SECONDS=8 cargo run -p saddle-physics-object-interaction-lab
```

With E2E:

```bash
cargo run -p saddle-physics-object-interaction-lab --features e2e -- object_interaction_smoke
```

## Controls

- `1` crate station
- `2` heavy station
- `3` inspect station
- `4` occlusion station
- `E` acquire
- `R` release
- `F` throw
- `Z` / `X` adjust hold distance
- `A` / `D` rotate held object
- `Tab` / `Q` cycle candidates

## BRP

Run the lab, then inspect from another terminal:

```bash
uv run --project .codex/skills/bevy-brp/script brp ping
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_physics_object_interaction::debug::ObjectInteractionDiagnostics
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_physics_object_interaction_lab::common::DemoDiagnostics
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/object_interaction_lab.png
uv run --project .codex/skills/bevy-brp/script brp extras shutdown
```
