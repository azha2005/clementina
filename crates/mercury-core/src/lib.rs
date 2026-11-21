//! Núcleo del emulador de la **Ferranti Mercury**, el modelo de *Clementina*,
//! la primera computadora científica de la Argentina (Instituto de Cálculo,
//! FCEN-UBA, en servicio desde el 15 de mayo de 1961).
//!
//! Este crate no hace E/S: los periféricos se conectan con el trait [`io::Io`].

pub mod acc;
pub mod asm;
pub mod cpu;
pub mod io;
pub mod midi;
pub mod opcodes;
pub mod store;
pub mod tape;

pub use cpu::{Config, Machine, Stop};
