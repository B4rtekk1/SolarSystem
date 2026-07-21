<div align="center">

# 🪐 Solar System

### An interactive, real-time Solar System simulator and planetarium

[![Rust](https://img.shields.io/badge/Rust-2024-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![wgpu](https://img.shields.io/badge/GPU-wgpu-5C3EE8)](https://wgpu.rs/)
[![egui](https://img.shields.io/badge/UI-egui-4A90E2)](https://github.com/emilk/egui)

Explore planets and moons, observe their orbital motion, and analyze simulation data — all in a native GPU-rendered application.

</div>

---

## 🎬 DEMO


https://github.com/user-attachments/assets/b9040f11-5147-4250-a88f-fecba0a780d7



## ✨ Features

- **N-body simulation** with gravity, a fixed time step, and energy monitoring.
- **Detailed scene** containing the Sun, 8 planets, 5 dwarf planets, and 59 moons.
- **GPU rendering** powered by `wgpu` and WGSL shaders.
- **Interactive camera** with panning, orbiting, zooming, and selected-object tracking.
- **Celestial body selection and search** directly in the scene or through the object browser.
- **Configurable planet and moon orbits**, including adjustable path thickness.
- **Atmospheres, Saturn's rings, a starfield, and material effects** for a clear spatial visualization.
- **Energy and distance charts** for the entire system or the currently selected object.
- **Scene saving and loading** using the versioned `.orbs` format.
- **FPS counter**, fullscreen mode, and adjustable viewport size.

> [!NOTE]
> The model uses mean orbital elements suitable for an educational real-time simulation. It is not an astronomical ephemeris and should not be used to calculate precise celestial positions.

## 🚀 Quick start

### Requirements

- the latest stable version of [Rust](https://www.rust-lang.org/tools/install),
- a graphics card and drivers supported by `wgpu`,
- Git, if you are cloning the project from a repository.

### Running the application

```bash
git clone <repository-url>
cd SolarSystem
cargo run --release
```

The `--release` profile is recommended for smoother rendering and physics calculations. While developing, you can use the faster development build:

```bash
cargo run
```

## 🎮 Controls

| Action | Control |
|---|---|
| Select an object | left-click |
| Pan the view | left-click and drag |
| Orbit the camera around the scene | right-click and drag |
| Zoom in or out | mouse wheel |
| Toggle selected-object tracking | `C` |
| Clear the selection | `Esc` |
| Save to `solar_system.orbs` | `F5` |
| Load from `solar_system.orbs` | `F9` |
| Toggle fullscreen mode | `F11` |

The application panel also provides camera presets for a top-down view, an ecliptic view, resetting the camera, and focusing on the selected body.

## 💾 `.orbs` scene files

The simulation state can be saved from the application panel or by pressing `F5`. The file stores, among other things:

- celestial body positions and parameters,
- simulation state and speed,
- camera settings and the selected object,
- orbit visibility and path thickness,
- window size and reference data for energy measurements.

The format contains an `ORBS` header and a version number. Saves are written through a temporary file to reduce the risk of corrupting an existing scene. A sample `solar_system.orbs` scene is included in the project root.

## 🧰 Technology stack

| Area | Technology |
|---|---|
| Language | Rust 2024 |
| Rendering | `wgpu`, WGSL |
| Windowing and input | `winit` |
| User interface | `egui`, `egui-wgpu`, `egui-winit` |
| 3D mathematics | `glam` |
| Text rendering | `cosmic-text` |
| Serialization | `serde`, `serde_json` |
| File dialogs | `rfd` |

## 🗂️ Project structure

```text
SolarSystem/
├── assets/                 # fonts and application assets
├── src/
│   ├── shaders/            # WGSL shaders
│   ├── state/              # rendering, UI, and interactions
│   ├── app.rs              # window and event handling
│   ├── camera.rs           # 3D camera
│   ├── ecs.rs              # scene data model
│   ├── nbody.rs            # gravitational simulation
│   ├── scene.rs            # Solar System definition
│   ├── save.rs             # .orbs format and validation
│   └── main.rs             # application entry point
├── Cargo.toml
└── solar_system.orbs      # sample saved scene
```

## 🛠️ Development and testing

```bash
# Check the project
cargo check

# Run the test suite
cargo test

# Check formatting and common issues
cargo fmt --check
cargo clippy --all-targets --all-features

# Create an optimized build
cargo build --release
```

The test suite covers N-body physics, scene construction, orbit geometry, GPU data, and `.orbs` file persistence and validation.

## 📦 Publishing a release

Create and push a version tag to start the automated Windows release workflow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions will build the project in release mode, create a GitHub Release with automatically generated notes, and attach a ready-to-run `SolarSystem-<tag>-windows-x86_64.exe` file.
