//! n12panel - the graphical control panel for the Yamaha n-series.
//!
//! A small window that shows the current sample rate and lets you change it.
//! Changing the rate is a device restart (the driver only applies a rate at
//! start), so the work runs on a background thread and the window stays
//! responsive while `n12_on.sh --rate=HZ` does its thing.
//!
//! Run:  cargo run --release --bin n12panel

use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

use n12ctl::{coreaudio, host};

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
        };
        panel.refresh();
        panel
    }

    /// Re-read the device and its rates from CoreAudio.
    fn refresh(&mut self) {
        self.device = coreaudio::device_named("n12");
        let Some(id) = self.device else {
            self.current = None;
            self.rates.clear();
            return;
        };

        self.current = coreaudio::nominal_rate(id);

        // CoreAudio reports rates as ranges; here they are all single values, so
        // take the minimum, dedupe and sort.
        let mut rates: Vec<u32> = coreaudio::available_rates(id).iter().map(|r| r.min as u32).collect();
        rates.sort_unstable();
        rates.dedup();
        self.rates = rates;

        // Default the selection to the current rate, or the first offered one.
        if let Some(cur) = self.current {
            self.selected = cur as u32;
        } else if let Some(&first) = self.rates.first() {
            self.selected = first;
        }
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
            // Store the result for the UI thread to pick up next frame.
            *slot.lock().unwrap() = Some(outcome);
        });
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

impl eframe::App for Panel {
    // eframe 0.37 hands the app a `Ui` directly (there is no separate `update`).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        if self.busy {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }

        ui.heading("Yamaha n12");
        ui.add_space(4.0);

        // Buttons set flags rather than acting inside the closure, so `self` is
        // not borrowed mutably twice at once.
        let mut do_apply = false;
        let mut do_refresh = false;

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

        if do_apply {
            self.apply();
        }
        if do_refresh {
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

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([380.0, 320.0]),
        ..Default::default()
    };
    eframe::run_native(
        "n12 Control Panel",
        options,
        Box::new(|_cc| Ok(Box::new(Panel::new()))),
    )
}
