# Bevy Text3D (Standalone)

This is a standalone copy of the `bevy_text3d` crate (originally part of the `open_space_mmo` workspace) that has been adapted to be used as a separate crate without the optional `open_space_controller` dependency.

The crate exposes a Bevy plugin for rendering 3D text using Signed Distance Field (SDF) fonts, along with a simple example set and shadow-casting utilities.

## Quick Start

```bash
cargo run --example basic
```

Note: Examples have been modified to not require `open_space_controller`. They use basic Bevy cameras and UI for demonstration.

See the examples folder for more demos.
