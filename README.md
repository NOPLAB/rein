# rein

3D rendering library built on [wgpu](https://github.com/gfx-rs/wgpu).

## Features

- **wgpu backend** - Cross-platform graphics with WebGPU
- **PBR materials** - Physically-based rendering with Phong and PBR materials
- **Shadow mapping** - Directional light shadows
- **Post-processing effects** - FXAA, fog, and extensible effect chain
- **Camera controls** - Orbit, fly, and first-person controls
- **URDF support** - Load robot models from URDF files
- **Text rendering** - Optional GUI text rendering with glyphon
- **Instanced rendering** - Efficient rendering of many identical objects

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rein = { git = "https://github.com/NOPLAB/rein" }
```

### Feature Flags

| Feature  | Default | Description |
|----------|---------|-------------|
| `window` | Yes     | Window management with winit |
| `gui`    | No      | Text rendering with glyphon |

## Architecture

The library is organized into layers:

1. **context** - Core wgpu wrapper (Device, Queue)
2. **core** - Mid-level abstractions (buffers, textures, pipelines)
3. **renderer** - High-level rendering (cameras, materials, objects, lights)
4. **window** - Window management with winit (optional)
5. **gui** - Text rendering (optional)
6. **urdf** - URDF robot model support

## Example

```rust
use rein::{Window, WindowSettings, FrameOutput};
use rein::renderer::{Camera, OrbitControl};
use rein::urdf::RobotModel;
use rein::core::ClearState;
use glam::Vec3;

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("Robot Viewer"))?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        robot: Option<RobotModel>,
    }

    // Initialize and run your render loop...
    Ok(())
}
```

## Acknowledgments

This project is heavily inspired by [three-d](https://github.com/asny/three-d), a fantastic 3D rendering library for Rust. The architecture, API design, and many implementation patterns in rein are based on three-d's excellent work. We are deeply grateful to the three-d authors and contributors for creating such a well-designed and educational codebase.

## License

MIT License - see [LICENSE](LICENSE) for details.
