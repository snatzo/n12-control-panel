//! n12ctl - shared library for the command-line tool (`n12ctl`) and the GUI
//! panel (`n12panel`).
//!
//! Modules:
//! * [`iokit`]     - minimal IOKit FFI (find and open the Yamaha user client)
//! * [`n12`]       - the Yamaha driver's numbered methods, given names
//! * [`coreaudio`] - minimal CoreAudio FFI (device lookup, sample rates)
//! * [`host`]      - running n12-monterey's `n12_on.sh` to (re)start the device

pub mod coreaudio;
pub mod coremidi;
pub mod host;
pub mod iokit;
pub mod n12;
