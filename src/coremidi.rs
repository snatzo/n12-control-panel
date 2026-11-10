//! coremidi.rs - read the CoreMIDI endpoint names, to show the desk's ports.
//!
//! Tiny, dependency-free FFI to the CoreMIDI and CoreFoundation frameworks.
//! `MIDIObjectRef` is `UInt32` in CoreMIDI, so endpoints travel as plain `u32`s.

use std::ffi::c_void;

/// A `MIDIEndpointRef` (== `MIDIObjectRef` == `u32`).
pub type MidiEndpoint = u32;

#[link(name = "CoreMIDI", kind = "framework")]
unsafe extern "C" {
    /// The CFString property key for an endpoint's user-visible name.
    static kMIDIPropertyDisplayName: *const c_void;

    fn MIDIGetNumberOfSources() -> usize;
    fn MIDIGetSource(index: usize) -> MidiEndpoint;
    fn MIDIGetNumberOfDestinations() -> usize;
    fn MIDIGetDestination(index: usize) -> MidiEndpoint;

    fn MIDIObjectGetStringProperty(
        object: MidiEndpoint,
        property: *const c_void,
        out_string: *mut *const c_void,
    ) -> i32; // OSStatus
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringGetCString(
        the_string: *const c_void,
        buffer: *mut std::os::raw::c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
    fn CFRelease(cf: *const c_void);
}

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

/// The display name of one endpoint, or `None`.
fn endpoint_name(ep: MidiEndpoint) -> Option<String> {
    let mut cf: *const c_void = std::ptr::null();
    // SAFETY: we ask CoreMIDI for one CFStringRef, then copy it to a C buffer.
    unsafe {
        if MIDIObjectGetStringProperty(ep, kMIDIPropertyDisplayName, &mut cf) != 0 || cf.is_null() {
            return None;
        }
        let mut buf = [0i8; 256];
        let ok = CFStringGetCString(cf, buf.as_mut_ptr(), buf.len() as isize, CF_STRING_ENCODING_UTF8);
        CFRelease(cf);
        if ok == 0 {
            return None;
        }
        let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// All CoreMIDI source names (things that can send *to* us).
pub fn sources() -> Vec<String> {
    // SAFETY: plain CoreMIDI enumeration.
    unsafe {
        (0..MIDIGetNumberOfSources())
            .filter_map(|i| endpoint_name(MIDIGetSource(i)))
            .collect()
    }
}

/// All CoreMIDI destination names (things we can send *to*).
pub fn destinations() -> Vec<String> {
    // SAFETY: plain CoreMIDI enumeration.
    unsafe {
        (0..MIDIGetNumberOfDestinations())
            .filter_map(|i| endpoint_name(MIDIGetDestination(i)))
            .collect()
    }
}

/// Names that look like the desk (contain "n12", "mLAN" or "yamaha").
fn desk_like(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|n| {
            let l = n.to_lowercase();
            l.contains("n12") || l.contains("mlan") || l.contains("yamaha")
        })
        .cloned()
        .collect()
}

/// The desk's source names.
pub fn desk_sources() -> Vec<String> {
    desk_like(&sources())
}

/// The desk's destination names.
pub fn desk_destinations() -> Vec<String> {
    desk_like(&destinations())
}
