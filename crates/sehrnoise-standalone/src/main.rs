//! The standalone application.
//!
//! The application is the plugin without a host: the same engine, the same
//! parameters and the same panel, wrapped in a window of their own.
//!
//! The panel itself is the one in `sehrnoise-plugin`, so the two cannot
//! drift apart.

use nice_plug::prelude::*;
use sehrnoise_plugin::SehrNoise;

fn main() {
    nice_export_standalone::<SehrNoise>();
}
