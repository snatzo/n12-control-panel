//! iokit.rs - the smallest bit of IOKit we need, in safe-ish Rust.
//!
//! WHY THIS EXISTS
//!   The Yamaha audio driver publishes a service called `YamahaFWStreamDevice`.
//!   A user-space program talks to it by:
//!     1. finding the service by class name,
//!     2. opening a *user client* on it (a Mach port),
//!     3. calling numbered "methods" on that client with scalars and/or
//!        structures.
//!   That is what the C tools in n12-monterey do; this is the same thing in
//!   Rust, calling Apple's IOKit framework directly (no crates).
//!
//! READING NOTE (Rust concepts here)
//!   * `unsafe extern "C" { ... }` declares functions that live in a C library
//!     (here, the IOKit framework). `extern "C"` fixes the *calling convention*.
//!   * `#[link(name = "IOKit", kind = "framework")]` tells the linker which
//!     macOS framework provides those symbols.
//!   * Every call to such a function is `unsafe`, because Rust cannot check the
//!     pointer contract for us.

use std::ffi::CString;

// Mach/IOKit scalar types. `mach_port_t`, `io_service_t`, `io_connect_t` and
// `io_object_t` are all plain 32-bit unsigned integers.
pub type KernReturn = i32;
pub type MachPort = u32;
pub type IoService = u32;
pub type IoConnect = u32;
pub type IoObject = u32;

pub const KERN_SUCCESS: KernReturn = 0;

/// Opaque C types we only ever pass around by pointer.
pub enum CFDictionary {}
pub enum CFMutableDictionary {}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    /// Build a matching dictionary for a service class name (e.g.
    /// "YamahaFWStreamDevice"). The returned dictionary is *owned* by us.
    fn IOServiceMatching(name: *const std::os::raw::c_char) -> *mut CFMutableDictionary;

    /// Return the first service matching the dictionary. This *consumes* the
    /// dictionary (releases it), so we must not release it again.
    fn IOServiceGetMatchingService(
        main_port: MachPort,
        matching: *mut CFDictionary,
    ) -> IoService;

    /// Open a user client on a service. `client_type` is 0 for the default
    /// client. On success `*connect` receives the connection handle.
    fn IOServiceOpen(
        service: IoService,
        owning_task: MachPort,
        client_type: u32,
        connect: *mut IoConnect,
    ) -> KernReturn;

    /// Close a user client.
    fn IOServiceClose(connect: IoConnect) -> KernReturn;

    /// Release an IOKit object (service, iterator...). NOT for connections.
    fn IOObjectRelease(object: IoObject) -> KernReturn;

    /// Call a method whose inputs are scalars only.
    fn IOConnectCallScalarMethod(
        connect: IoConnect,
        selector: u32,
        input: *const u64,
        input_cnt: u32,
        output: *mut u64,
        output_cnt: *mut u32,
    ) -> KernReturn;

    /// Call a method with both scalars and a structure input.
    #[allow(clippy::too_many_arguments)]
    fn IOConnectCallMethod(
        connect: IoConnect,
        selector: u32,
        input: *const u64,
        input_cnt: u32,
        input_struct: *const std::os::raw::c_void,
        input_struct_cnt: usize,
        output: *mut u64,
        output_cnt: *mut u32,
        output_struct: *mut std::os::raw::c_void,
        output_struct_cnt: *mut usize,
    ) -> KernReturn;
}

// The current task's Mach port. On macOS this is a global variable, and the
// familiar `mach_task_self()` is a macro that reads it.
unsafe extern "C" {
    static mach_task_self_: MachPort;
}

/// `kIOMainPortDefault` (macOS 12+) and `kIOMasterPortDefault` are both 0.
const MAIN_PORT: MachPort = 0;

/// An open user client on the Yamaha service. When this value is dropped, the
/// connection and the service are closed for us.
///
/// This is Rust's RAII in action: `Drop` is the destructor, and it is the
/// idiomatic place to release resources that were acquired in `open`.
pub struct UserClient {
    connect: IoConnect,
    service: IoService,
}

impl UserClient {
    /// Find `YamahaFWStreamDevice` and open a user client on it.
    ///
    /// Returns `None` if the service is not present (the Yamaha kexts are not
    /// loaded) or the open fails.
    pub fn open() -> Option<UserClient> {
        let name = CString::new("YamahaFWStreamDevice").ok()?;

        // SAFETY: `name` is a valid NUL-terminated C string for the duration of
        // the call; IOServiceMatching returns a dictionary or NULL.
        let service = unsafe {
            let matching = IOServiceMatching(name.as_ptr());
            if matching.is_null() {
                return None;
            }
            IOServiceGetMatchingService(MAIN_PORT, matching as *mut CFDictionary)
        };
        if service == 0 {
            return None;
        }

        let mut connect: IoConnect = 0;
        // SAFETY: `connect` is a valid local for the out-parameter.
        let kr = unsafe {
            let task = mach_task_self_;
            IOServiceOpen(service, task, 0, &mut connect)
        };
        if kr != KERN_SUCCESS || connect == 0 {
            // Opening failed: release the service we found.
            unsafe { IOObjectRelease(service) };
            return None;
        }

        Some(UserClient { connect, service })
    }

    /// Call a scalar-only method (the common case on this driver).
    ///
    /// `inputs` are the scalar inputs; the returned pair is
    /// `(kernel_result, output_scalars)`. The number of outputs we ask for is
    /// the length of `nout`.
    pub fn call_scalar(&self, selector: u32, inputs: &[u64], nout: usize) -> (KernReturn, Vec<u64>) {
        let mut out = vec![0u64; nout];
        let mut out_cnt = nout as u32;
        // SAFETY: slices outlive the call; we pass matching counts.
        let kr = unsafe {
            IOConnectCallScalarMethod(
                self.connect,
                selector,
                inputs.as_ptr(),
                inputs.len() as u32,
                out.as_mut_ptr(),
                &mut out_cnt,
            )
        };
        out.truncate(out_cnt as usize);
        (kr, out)
    }

    /// Call a method with scalars *and* a structure input (e.g. an array of
    /// sample rates).
    pub fn call_with_struct_in(
        &self,
        selector: u32,
        inputs: &[u64],
        struct_in: &[u8],
    ) -> KernReturn {
        let mut out_cnt: u32 = 0;
        // SAFETY: pointers/counts match the slices.
        unsafe {
            IOConnectCallMethod(
                self.connect,
                selector,
                inputs.as_ptr(),
                inputs.len() as u32,
                struct_in.as_ptr() as *const std::os::raw::c_void,
                struct_in.len(),
                std::ptr::null_mut(),
                &mut out_cnt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        }
    }

    /// The raw connection handle, for callers that need it.
    #[allow(dead_code)]
    pub fn raw(&self) -> IoConnect {
        self.connect
    }
}

impl Drop for UserClient {
    fn drop(&mut self) {
        // SAFETY: both handles were created by our `open`, and are closed once.
        unsafe {
            IOServiceClose(self.connect);
            IOObjectRelease(self.service);
        }
    }
}
