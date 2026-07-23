# Solar System

Solar System is an interactive, real-time simulator and planetarium written in Rust. It combines an N-body gravity simulation with native GPU rendering to visualize the Solar System and inspect simulation data.

[Watch the demo](https://github.com/user-attachments/assets/b9040f11-5147-4250-a88f-fecba0a780d7)

## Features

- N-body gravity simulation with a fixed time step and energy monitoring
- the Sun, 8 planets, 5 dwarf planets, and 59 moons
- GPU rendering with atmospheres, Saturn's rings, orbital paths, and a starfield
- object selection, search, camera controls, and selected-object tracking
- configurable simulation speed, orbit visibility, and path thickness
- energy and distance charts
- saving and loading simulation state in the versioned `.orbs` format
- fullscreen mode, adjustable viewport size, and an FPS counter

The model uses mean orbital elements and is intended for educational visualization. It is not an astronomical ephemeris and should not be used to calculate precise celestial positions.

## Requirements

- the latest stable version of [Rust](https://www.rust-lang.org/tools/install)
- a graphics card and drivers supported by [`wgpu`](https://wgpu.rs/)
- Git, when cloning the repository

## Running

```bash
git clone https://github.com/B4rtekk1/SolarSystem.git
cd SolarSystem
cargo run --release
```

The release profile is recommended for smoother rendering and physics calculations. For a faster development build, run `cargo run`.

## Controls

| Action | Control |
|---|---|
| Select an object | left-click |
| Pan the view | left-click and drag |
| Orbit the camera | right-click and drag |
| Zoom | mouse wheel |
| Toggle selected-object tracking | `C` |
| Clear the selection | `Esc` |
| Save to `solar_system.orbs` | `F5` |
| Load from `solar_system.orbs` | `F9` |
| Toggle fullscreen mode | `F11` |

The application panel also includes camera presets, object search, simulation controls, and scene save/load actions.

## Scene files

The `.orbs` format stores celestial body data, simulation state and speed, camera settings, selection, orbit display settings, window size, and energy reference data. Files include an `ORBS` header and a format version. A sample `solar_system.orbs` scene is included in the project root.

## Technology

- Rust 2024
- `wgpu` and WGSL for rendering
- `winit` for windowing and input
- `egui` for the user interface
- `glam` for 3D mathematics
- `serde` and `serde_json` for serialization

## Development

```bash
cargo check
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features
cargo build --release
```

The test suite covers N-body physics, scene construction, orbit geometry, GPU data, and `.orbs` persistence and validation.
