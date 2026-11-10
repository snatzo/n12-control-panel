//! coreaudio.rs - the small slice of CoreAudio needed to find the n12 device
//! and read/set its sample rate.
//!
//! WHY THIS EXISTS
//!   Changing the rate happens at the *driver* level (see `host`), but reading
//!   which rates the device offers and which one is current is easiest through
//!   CoreAudio's device properties. This is a dependency-free FFI to the
//!   `CoreAudio` and `CoreFoundation` frameworks.
//!
//! CoreAudio property selectors are four-character codes; `fourcc` turns a
//! literal like `b"dev#"` into the `u32` the API expects.

use std::ffi::c_void;

/// `AudioObjectID` / `AudioDeviceID`.
pub type AudioObjectId = u32;
/// `OSStatus`.
pub type OsStatus = i32;

/// The system object that owns the list of devices.
const OBJECT_SYSTEM: AudioObjectId = 1;

/// `kAudioObjectPropertyScopeGlobal` = 'glob'.
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
/// `kAudioObjectPropertyElementMain` = 0.
const ELEMENT_MAIN: u32 = 0;

const PROP_DEVICES: u32 = fourcc(b"dev#");
const PROP_NAME: u32 = fourcc(b"lnam");
const PROP_NOMINAL_RATE: u32 = fourcc(b"nsrt");
const PROP_AVAILABLE_RATES: u32 = fourcc(b"nsr#");

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

/// Build the `u32` a selector or scope uses, from its four characters.
const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}

/// `AudioObjectPropertyAddress` - what property, in which scope, on which element.
#[repr(C)]
#[derive(Clone, Copy)]
struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

/// `AudioValueRange` - CoreAudio reports available rates as ranges.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object: AudioObjectId,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        out_size: *mut u32,
    ) -> OsStatus;
    fn AudioObjectGetPropertyData(
        object: AudioObjectId,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        io_size: *mut u32,
        out_data: *mut c_void,
    ) -> OsStatus;
    fn AudioObjectSetPropertyData(
        object: AudioObjectId,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        data_size: u32,
        data: *const c_void,
    ) -> OsStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringGetCString(
        the_string: *const c_void,
        buffer: *mut std::os::raw::c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8; // Boolean
    fn CFRelease(cf: *const c_void);
}

/// Every audio device the system knows about.
pub fn all_devices() -> Vec<AudioObjectId> {
    let addr = PropertyAddress { selector: PROP_DEVICES, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    let mut size: u32 = 0;
    // SAFETY: standard property query; `size` is a valid out-parameter.
    unsafe {
        if AudioObjectGetPropertyDataSize(OBJECT_SYSTEM, &addr, 0, std::ptr::null(), &mut size) != 0
            || size == 0
        {
            return Vec::new();
        }
        let mut ids = vec![0u32; size as usize / 4];
        if AudioObjectGetPropertyData(OBJECT_SYSTEM, &addr, 0, std::ptr::null(), &mut size, ids.as_mut_ptr() as *mut c_void) != 0
        {
            return Vec::new();
        }
        ids.truncate(size as usize / 4);
        ids
    }
}

/// The user-visible name of a device, e.g. "n12".
pub fn device_name(device: AudioObjectId) -> Option<String> {
    let addr = PropertyAddress { selector: PROP_NAME, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    let mut size: u32 = std::mem::size_of::<*const c_void>() as u32;
    let mut cf: *const c_void = std::ptr::null();
    // SAFETY: we ask for one CFStringRef into `cf`, then copy it to a C buffer.
    unsafe {
        if AudioObjectGetPropertyData(device, &addr, 0, std::ptr::null(), &mut size, &mut cf as *mut _ as *mut c_void) != 0
            || cf.is_null()
        {
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

/// The first device whose name contains `needle` (case-insensitive).
pub fn device_named(needle: &str) -> Option<AudioObjectId> {
    let want = needle.to_lowercase();
    all_devices().into_iter().find(|&id| {
        device_name(id).map(|n| n.to_lowercase().contains(&want)).unwrap_or(false)
    })
}

/// The device's current sample rate in Hz.
pub fn nominal_rate(device: AudioObjectId) -> Option<f64> {
    let addr = PropertyAddress { selector: PROP_NOMINAL_RATE, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    let mut rate: f64 = 0.0;
    let mut size: u32 = std::mem::size_of::<f64>() as u32;
    // SAFETY: one f64 out.
    unsafe {
        if AudioObjectGetPropertyData(device, &addr, 0, std::ptr::null(), &mut size, &mut rate as *mut _ as *mut c_void) != 0 {
            return None;
        }
    }
    Some(rate)
}

/// The sample rates the device offers.
pub fn available_rates(device: AudioObjectId) -> Vec<ValueRange> {
    let addr = PropertyAddress { selector: PROP_AVAILABLE_RATES, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    let mut size: u32 = 0;
    // SAFETY: size query then a read of that many bytes into the Vec.
    unsafe {
        if AudioObjectGetPropertyDataSize(device, &addr, 0, std::ptr::null(), &mut size) != 0 || size == 0 {
            return Vec::new();
        }
        let count = size as usize / std::mem::size_of::<ValueRange>();
        let mut ranges = vec![ValueRange { min: 0.0, max: 0.0 }; count];
        if AudioObjectGetPropertyData(device, &addr, 0, std::ptr::null(), &mut size, ranges.as_mut_ptr() as *mut c_void) != 0 {
            return Vec::new();
        }
        ranges.truncate(size as usize / std::mem::size_of::<ValueRange>());
        ranges
    }
}

/// Ask CoreAudio to set the rate. NOTE: on this device it returns success but
/// does not take effect - the rate is applied at driver start (see `host`).
pub fn set_nominal_rate(device: AudioObjectId, hz: f64) -> OsStatus {
    let addr = PropertyAddress { selector: PROP_NOMINAL_RATE, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    // SAFETY: one f64 in.
    unsafe { AudioObjectSetPropertyData(device, &addr, 0, std::ptr::null(), std::mem::size_of::<f64>() as u32, &hz as *const _ as *const c_void) }
}
