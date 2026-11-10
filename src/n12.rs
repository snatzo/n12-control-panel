//! n12.rs - the Yamaha driver's numbered methods, given names.
//!
//! The driver exposes one user-client method per number ("selector"). These
//! numbers come from the driver's method table and were confirmed by watching
//! the vendor software call them. Only the ones this tool needs are listed.
//!
//! Types: everything is a `u32`/`u64` at the boundary, so the wrappers here
//! turn those into friendlier shapes (`u32` sample rate, `Vec<u32>` of rates).

use crate::iokit::{KernReturn, UserClient, KERN_SUCCESS};

/// Method selectors on `YamahaFWStreamDevice`.
pub const SEL_OPEN: u32 = 0;
pub const SEL_GET_NUM_AUDIO_DEV: u32 = 5;
pub const SEL_GET_SYT_SYNC: u32 = 42;
// Kept (but unused so far) so the set of known selectors is complete and the
// next milestone can call them without re-deriving the numbers.
#[allow(dead_code)]
pub const SEL_GET_NUM_AUDIO_STREAMS: u32 = 13;
#[allow(dead_code)]
pub const SEL_SET_SYT_SYNC: u32 = 43;
pub const SEL_GET_MAX_SPEED: u32 = 44;
pub const SEL_SET_AVAILABLE_RATES: u32 = 76;
pub const SEL_GET_SAMPLE_RATE: u32 = 77;
pub const SEL_GET_MASTER_CLOCK: u32 = 78;
pub const SEL_GET_CLOCK_SOURCE: u32 = 79;
pub const SEL_GET_AVAILABLE_RATES: u32 = 82;
// MIDI
#[allow(dead_code)]
pub const SEL_CREATE_MIDI_DEVICE: u32 = 18;
#[allow(dead_code)]
pub const SEL_START_MIDI_DEVICE: u32 = 25;
#[allow(dead_code)]
pub const SEL_CREATE_MIDI_PLUG: u32 = 27;
pub const SEL_GET_NUM_MIDI_DEVICES: u32 = 20;
pub const SEL_GET_NUM_MIDI_PLUGS: u32 = 29;
pub const SEL_SET_MIDI_MODEL_ID: u32 = 67;

/// The driver's own default rate list (`s_availableSampleRatesDefault`).
pub const DEFAULT_RATES: [u32; 4] = [44100, 48000, 88200, 96000];

/// A thin, typed façade over an opened [`UserClient`].
pub struct Device {
    client: UserClient,
}

impl Device {
    /// Open the driver and issue `sOpen`. Returns `None` if the kexts are not
    /// loaded or the device refuses to open.
    pub fn open() -> Option<Device> {
        let client = UserClient::open()?;
        let (kr, _) = client.call_scalar(SEL_OPEN, &[], 0);
        if kr != KERN_SUCCESS {
            return None;
        }
        Some(Device { client })
    }

    /// The device's current sample rate in Hz, or `None` on error.
    pub fn sample_rate(&self) -> Option<u32> {
        let (kr, out) = self.client.call_scalar(SEL_GET_SAMPLE_RATE, &[], 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }

    /// The rates the driver will advertise, read from its own list.
    ///
    /// `sGetAvailableSampleRates` is called twice: once with a count of 0 to
    /// learn how many rates there are, then with a buffer big enough to hold
    /// them. The buffer's address is passed as the second scalar, because the
    /// driver copies the list straight into the caller's memory.
    pub fn available_rates(&self) -> Option<Vec<u32>> {
        let (kr, out) = self
            .client
            .call_scalar(SEL_GET_AVAILABLE_RATES, &[0, 0], 1);
        if kr != KERN_SUCCESS {
            return None;
        }
        let count = out.first().copied().unwrap_or(0) as usize;
        if count == 0 {
            return Some(Vec::new());
        }

        let mut buf = vec![0u32; count];
        let addr = buf.as_mut_ptr() as u64;
        let (kr2, out2) = self.client.call_scalar(
            SEL_GET_AVAILABLE_RATES,
            &[count as u64, addr],
            1,
        );
        if kr2 != KERN_SUCCESS {
            return None;
        }
        let got = out2.first().copied().unwrap_or(0) as usize;
        buf.truncate(got.min(count));
        Some(buf)
    }

    /// Ask the driver to offer `rates` (max 8; each must be one of 32000,
    /// 44100, 48000, 88200, 96000).
    pub fn set_available_rates(&self, rates: &[u32]) -> KernReturn {
        // Pack the u32 rates into the byte array the structure input expects.
        let mut bytes = Vec::with_capacity(rates.len() * 4);
        for r in rates {
            bytes.extend_from_slice(&r.to_le_bytes());
        }
        let scalar_in = [rates.len() as u64];
        self.client
            .call_with_struct_in(SEL_SET_AVAILABLE_RATES, &scalar_in, &bytes)
    }

    /// Current clock source index (`sGetClockSource`).
    pub fn clock_source(&self) -> Option<u32> {
        let (kr, out) = self.client.call_scalar(SEL_GET_CLOCK_SOURCE, &[0], 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }

    /// Master-clock index (`sGetMasterClock`).
    pub fn master_clock(&self) -> Option<u32> {
        let (kr, out) = self.client.call_scalar(SEL_GET_MASTER_CLOCK, &[], 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }

    /// The isochronous channel the node derives its clock from (`sGetSYTSynchChannel`).
    pub fn syt_sync_channel(&self) -> Option<u32> {
        let (kr, out) = self.client.call_scalar(SEL_GET_SYT_SYNC, &[], 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }

    /// FireWire bus speed (`sGetMaxSpeed`).
    pub fn max_speed(&self) -> Option<u32> {
        let (kr, out) = self.client.call_scalar(SEL_GET_MAX_SPEED, &[], 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }

    /// How many audio devices the driver has (`sGetNumAudioDevices`).
    pub fn num_audio_devices(&self) -> Option<u32> {
        let (kr, out) = self.client.call_scalar(SEL_GET_NUM_AUDIO_DEV, &[], 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }

    /// How many MIDI devices the driver has (`sGetNumMIDIDevices`).
    pub fn num_midi_devices(&self) -> Option<u32> {
        let (kr, out) = self.client.call_scalar(SEL_GET_NUM_MIDI_DEVICES, &[], 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }

    /// How many MIDI ports (plugs) device `index` has in one direction.
    pub fn num_midi_plugs(&self, index: u32, is_input: bool) -> Option<u32> {
        let args = [index as u64, if is_input { 1 } else { 0 }];
        let (kr, out) = self.client.call_scalar(SEL_GET_NUM_MIDI_PLUGS, &args, 1);
        if kr == KERN_SUCCESS {
            out.first().map(|v| *v as u32)
        } else {
            None
        }
    }
}

/// Format a kernel result for humans: the number and, if it looks like one, a
/// four-character code (so `0xe00002c2` reads as `kIOReturnBadArgument`).
pub fn fmt_kr(kr: KernReturn) -> String {
    let v = kr as u32;
    if (0x20..=0x7e).contains(&(v >> 24))
        && (0x20..=0x7e).contains(&(v >> 16 & 0xff))
        && (0x20..=0x7e).contains(&(v >> 8 & 0xff))
        && (0x20..=0x7e).contains(&(v & 0xff))
    {
        let b = v.to_be_bytes();
        format!("0x{:08x} ('{}{}{}{}')", v, b[0] as char, b[1] as char, b[2] as char, b[3] as char)
    } else {
        format!("0x{:08x}", v)
    }
}
