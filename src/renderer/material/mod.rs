//! Material abstractions
//!
//! Provides material types for controlling surface appearance.

mod color;
mod common;
mod depth;
mod grid;
mod line;
mod normal;
mod pbr;
mod phong;
mod position;
mod sprite;
mod terrain;
mod traits;
mod unlit;
mod uv;

pub use color::ColorMaterial;
pub use depth::DepthMaterial;
pub use grid::GridMaterial;
pub use line::LineMaterial;
pub use normal::NormalMaterial;
pub use pbr::PbrMaterial;
pub use phong::PhongMaterial;
pub use position::PositionMaterial;
pub use sprite::SpriteMaterial;
pub use terrain::{TerrainMaterial, TerrainUniform};
pub use traits::{Material, ModelUniform};
pub use unlit::UnlitMaterial;
pub use uv::UVMaterial;
