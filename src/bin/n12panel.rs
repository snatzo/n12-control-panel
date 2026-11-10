//! n12panel - the graphical control panel for the Yamaha n-series.
//!
//! A small window that shows the current sample rate and the desk's MIDI state,
//! and lets you change the rate. Changing the rate is a device restart (the
//! driver only applies a rate at start), so the work runs on a background thread
//! and the window stays responsive while `n12_on.sh --rate=HZ` does its thing.
//!
//! Run:  cargo run --release --bin n12panel

use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

use n12ctl::n12::Device;
use n12ctl::{coreaudio, coremidi, host};

/// The window's state.
struct Panel {
    /// CoreAudio device id for the n12, if it is present.
    device: Option<u32>,
    /// The rate CoreAudio reports right now.
    current: Option<f64>,
    /// The rates the device offers (from CoreAudio).
    rates: Vec<u32>,
    /// The user's choice in the radio list.
    selected: u32,
    /// A line of feedback shown under the buttons.
    status: String,
    /// True while a rate change is in flight.
    busy: bool,
    /// Where the background thread parks its result.
    result: Arc<Mutex<Option<Result<(), String>>>>,

    /// MIDI: the driver's device count and the CoreMIDI port names.
    midi_devices: Option<u32>,
    midi_sources: Vec<String>,
    midi_destinations: Vec<String>,
}

impl Panel {
    fn new() -> Self {
        let mut panel = Panel {
            device: None,
            current: None,
            rates: Vec::new(),
            selected: 44100,
            status: String::new(),
            busy: false,
            result: Arc::new(Mutex::new(None)),
            midi_devices: None,
            midi_sources: Vec::new(),
            midi_destinations: Vec::new(),
        };
        panel.refresh();
        panel
    }

    /// Re-read the device, its rates, and the MIDI state.
    fn refresh(&mut self) {
        // --- audio ---
        self.device = coreaudio::device_named("n12");
        if let Some(id) = self.device {
            self.current = coreaudio::nominal_rate(id);
            // CoreAudio reports rates as ranges; here they are single values, so
            // take the minimum, dedupe and sort.
            let mut rates: Vec<u32> = coreaudio::available_rates(id).iter().map(|r| r.min as u32).collect();
            rates.sort_unstable();
            rates.dedup();
            self.rates = rates;
            if let Some(cur) = self.current {
                self.selected = cur as u32;
            } else if let Some(&first) = self.rates.first() {
                self.selected = first;
            }
        } else {
            self.current = None;
            self.rates.clear();
        }

        // --- MIDI ---
        // The driver's own MIDI device count (opening a fresh user client each
        // refresh is cheap); then what CoreMIDI has published.
        self.midi_devices = Device::open().and_then(|d| d.num_midi_devices());
        self.midi_sources = coremidi::desk_sources();
        self.midi_destinations = coremidi::desk_destinations();
    }

    /// Kick off a rate change on a background thread.
    fn apply(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = format!("restarting the n12 at {} Hz ...", self.selected);

        let hz = self.selected;
        let slot = Arc::clone(&self.result);
        thread::spawn(move || {
            let outcome = host::rate_restart(hz);
            *slot.lock().unwrap() = Some(outcome);
        });
    }

    /// Restart MIDIServer so CoreMIDI re-reads the desk's ports. Quick enough to
    /// do on the UI thread, and it makes the MIDI row update immediately after.
    fn midi_reinit(&mut self) {
        let _ = std::process::Command::new("/usr/bin/killall")
            .arg("MIDIServer")
            .status();
        self.status = "restarted MIDIServer".to_string();
    }

    /// Pick up a finished background job, if there is one.
    fn poll(&mut self) {
        let taken = self.result.lock().unwrap().take();
        if let Some(outcome) = taken {
            self.busy = false;
            self.refresh();
            self.status = match outcome {
                Ok(()) => format!("now running at {} Hz", self.selected),
                Err(e) => format!("failed: {}", e),
            };
        }
    }
}

impl Panel {
    /// The window's contents. Kept as its own method so the `Frame` wrapper in
    /// `ui()` can call it without tangling borrows.
    fn draw(&mut self, ui: &mut egui::Ui) {
        ui.heading("Yamaha n12");
        ui.add_space(4.0);

        // Buttons set flags rather than acting inside the closure, so `self` is
        // not borrowed mutably twice at once.
        let mut do_apply = false;
        let mut do_refresh = false;
        let mut do_midi_reinit = false;

        // --- audio ---
        match self.device {
            None => {
                ui.colored_label(
                    egui::Color32::from_rgb(0xb0, 0x40, 0x00),
                    "the n12 is not present - start it with n12-monterey's n12_on.sh",
                );
            }
            Some(id) => {
                let cur = self
                    .current
                    .map(|r| format!("{} Hz", r as u32))
                    .unwrap_or_else(|| "unknown".into());
                ui.label(format!("device id {}    current rate: {}", id, cur));
                ui.separator();

                ui.label("Sample rate:");
                for &r in &self.rates {
                    ui.radio_value(&mut self.selected, r, format!("{} Hz", r));
                }
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.busy, egui::Button::new("Apply")).clicked() {
                        do_apply = true;
                    }
                    if ui.add_enabled(!self.busy, egui::Button::new("Refresh")).clicked() {
                        do_refresh = true;
                    }
                });
            }
        }

        // --- MIDI ---
        ui.separator();
        ui.label("MIDI:");

        let have_ports = !self.midi_sources.is_empty() || !self.midi_destinations.is_empty();
        if have_ports {
            // The desk usually shows one in and one out, both named alike.
            let names: Vec<String> = self
                .midi_sources
                .iter()
                .chain(self.midi_destinations.iter())
                .cloned()
                .collect();
            let mut unique = names;
            unique.sort();
            unique.dedup();
            ui.label(format!("ports: {}", unique.join(", ")));
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(0xb0, 0x40, 0x00),
                "no ports (is n12_agent running with MIDI?)",
            );
        }
        match self.midi_devices {
            Some(n) => ui.label(format!("driver MIDI devices: {}", n)),
            None => ui.label("driver MIDI devices: (unavailable)"),
        };
        if ui
            .add_enabled(!self.busy, egui::Button::new("Re-read MIDI"))
            .clicked()
        {
            do_midi_reinit = true;
        }

        // --- act on the flags, outside any closure that borrows `self` ---
        if do_apply {
            self.apply();
        }
        if do_refresh {
            self.refresh();
        }
        if do_midi_reinit {
            self.midi_reinit();
            // Give MIDIServer a moment, then re-read the port list.
            std::thread::sleep(std::time::Duration::from_millis(400));
            self.refresh();
        }

        if !self.status.is_empty() {
            ui.add_space(6.0);
            ui.label(&self.status);
        }

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Changing the rate restarts the device; the desk switches with it.")
                .small()
                .weak(),
        );
    }
}

impl eframe::App for Panel {
    // eframe 0.37 hands the app a `Ui` directly (there is no separate `update`).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        if self.busy {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }

        // A margin keeps the content off the window edges.
        egui::Frame::new()
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| self.draw(ui));
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 460.0]),
        ..Default::default()
    };
    eframe::run_native(
        "n12 Control Panel",
        options,
        Box::new(|_cc| Ok(Box::new(Panel::new()))),
    )
}
