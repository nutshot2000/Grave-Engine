//! Grave Engine: a small 2D material-simulation engine.
//!
//! Everything (materials, scenes) is plain text, so people and AI agents can
//! both read and write it.

pub mod backdrop;
pub mod camera;
pub mod edit;
pub mod headless;
pub mod import;
pub mod light;
pub mod look;
pub mod notes;
pub mod physics;
pub mod sim;
pub mod template;
pub mod trigger;
pub mod material;
pub mod render;
pub mod scene;
pub mod world;

pub use material::{MatId, Material, Materials, State};
pub use sim::Sim;
pub use world::World;
