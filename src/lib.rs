pub mod arsenal;
pub mod buggy;
pub mod fracture;
pub mod rubble;
pub mod vehicle_gun;
mod wire;
pub use wire::PROTOCOL;

#[cfg(feature = "client")]
mod world_wire;
#[cfg(feature = "client")]
pub mod net;
