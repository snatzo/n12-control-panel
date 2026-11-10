//! n12ctl - a control panel for the Yamaha n-series (n12/n8) on modern macOS.
//!
//! Milestone 1: read the driver's state (sample rate, available rates, clock,
//! sync) and ask it to offer a different set of sample rates. This is also the
//! instrument for finishing the investigation: `rates` prints what the driver
//! itself believes, which tells us whether a rate-list change reached it.
//!
//! Run:
//!     n12ctl                 # print the driver's state
//!     n12ctl rates           # just the advertised sample rates
//!     n12ctl set-rates 44100 48000 88200 96000
//!
//! This needs the Yamaha kexts loaded; it does NOT need the audio device to be
//! started (n12-monterey's n12_agent can do that separately).

mod iokit;
mod n12;

use n12::{fmt_kr, Device, DEFAULT_RATES};

fn print_info(dev: &Device) {
    println!("Yamaha n-series driver");

    match dev.num_audio_devices() {
        Some(n) => println!("  audio devices              : {}", n),
        None => println!("  audio devices              : (query failed)"),
    }
    match dev.sample_rate() {
        Some(r) => println!("  current sample rate        : {} Hz", r),
        None => println!("  current sample rate        : (query failed)"),
    }
    match dev.available_rates() {
        Some(rates) if !rates.is_empty() => {
            let list: Vec<String> = rates.iter().map(|r| format!("{}", r)).collect();
            println!("  available sample rates     : {} Hz", list.join(", "));
        }
        Some(_) => println!("  available sample rates     : (list is empty)"),
        None => println!("  available sample rates     : (query failed)"),
    }
    match dev.clock_source() {
        Some(c) => println!("  clock source               : {}", c),
        None => println!("  clock source               : (query failed)"),
    }
    match dev.master_clock() {
        Some(c) => println!("  master clock               : {}", c),
        None => println!("  master clock               : (query failed)"),
    }
    match dev.syt_sync_channel() {
        Some(c) if c == 0xffff_ffff => {
            println!("  clock-sync channel (SYT)   : master (this node free-runs)")
        }
        Some(c) => println!("  clock-sync channel (SYT)   : slaved to isoch channel {}", c),
        None => println!("  clock-sync channel (SYT)   : (query failed)"),
    }
    match dev.max_speed() {
        Some(s) => println!("  FireWire speed             : {} (S{})", s, s * 100),
        None => println!("  FireWire speed             : (query failed)"),
    }
}

fn print_rates(dev: &Device) {
    match dev.available_rates() {
        Some(rates) if rates.is_empty() => {
            println!("the driver reports no available rates (list is empty)")
        }
        Some(rates) => {
            println!("the driver offers these sample rates:");
            for r in rates {
                println!("  {} Hz", r);
            }
        }
        None => println!("could not read the available rates"),
    }
    if let Some(r) = dev.sample_rate() {
        println!("current rate: {} Hz", r);
    }
}

fn set_rates(dev: &Device, rates: &[u32]) {
    if rates.is_empty() {
        eprintln!("give at least one rate, e.g.  n12ctl set-rates 44100 48000");
        std::process::exit(2);
    }
    let kr = dev.set_available_rates(rates);
    println!("sSetAvailableSampleRates({:?}) -> {}", rates, fmt_kr(kr));
    if kr == iokit::KERN_SUCCESS {
        print_rates(dev);
    } else {
        eprintln!(
            "the driver refused. Valid rates are 32000, 44100, 48000, 88200, 96000 (max 8)."
        );
    }
}

/// Locate the n12-monterey tools directory, which owns the device bring-up.
///
/// The control panel does not build the device itself yet, so changing the
/// sample rate means restarting the device through n12_on.sh --rate=N. Look
/// for the tools in, in order: $N12_TOOLS, a sibling checkout, the CWD.
fn n12_tools_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    if let Ok(p) = std::env::var("N12_TOOLS") {
        let dir = PathBuf::from(p);
        if dir.join("n12_on.sh").exists() {
            return Some(dir);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        // .../n12_control_panel/target/<profile>/n12ctl -> the projects folder
        let mut dir = exe.as_path();
        for _ in 0..4 {
            match dir.parent() {
                Some(p) => dir = p,
                None => break,
            }
        }
        candidates.push(dir.join("n12-monterey/tools"));
    }
    candidates.push(PathBuf::from("../n12-monterey/tools"));
    candidates.push(PathBuf::from("n12-monterey/tools"));
    candidates.into_iter().find(|c| c.join("n12_on.sh").exists())
}

/// Change the sample rate by restarting the device at that rate. The driver
/// only applies a new rate when the device (re)starts, so a rate change is a
/// stop-and-start; n12_on.sh --rate=N does exactly that.
fn rate_restart(hz: u32) {
    let tools = match n12_tools_dir() {
        Some(t) => t,
        None => {
            eprintln!("could not find n12-monterey's tools (set N12_TOOLS=/path/to/n12-monterey/tools)");
            std::process::exit(1);
        }
    };
    let script = tools.join("n12_on.sh");
    println!("restarting the n12 at {} Hz ...", hz);
    match std::process::Command::new("/bin/zsh")
        .arg(&script)
        .arg(format!("--rate={}", hz))
        .status()
    {
        Ok(s) if s.success() => println!("done - the n12 should now be at {} Hz", hz),
        Ok(s) => eprintln!("n12_on.sh exited with {}", s),
        Err(e) => eprintln!("could not run {}: {}", script.display(), e),
    }
}

fn main() {
    // `args()` skips argv[0]; collect the rest as owned Strings.
    let args: Vec<String> = std::env::args().skip(1).collect();

    let dev = match Device::open() {
        Some(d) => d,
        None => {
            eprintln!("could not open the YamahaFWStreamDevice service.");
            eprintln!("Are the Yamaha kexts loaded?  (see n12-monterey: sudo tools/try_kextload.sh --bypass)");
            std::process::exit(1);
        }
    };

    match args.first().map(|s| s.as_str()).unwrap_or("info") {
        "info" => print_info(&dev),
        "rates" => print_rates(&dev),
        "defaults" => set_rates(&dev, &DEFAULT_RATES),
        "rate" => {
            // Change the actual rate: a restart at that rate (see rate_restart).
            match args.get(1).and_then(|s| s.parse::<u32>().ok()) {
                Some(hz) => rate_restart(hz),
                None => {
                    eprintln!("usage: n12ctl rate HZ     (e.g. n12ctl rate 48000)");
                    std::process::exit(2);
                }
            }
        }
        "set-rates" => {
            // Parse every remaining argument as a number. `filter_map` skips
            // anything that does not parse, so a typo is ignored rather than
            // crashing.
            let rates: Vec<u32> = args
                .iter()
                .skip(1)
                .filter_map(|s| s.parse::<u32>().ok())
                .collect();
            set_rates(&dev, &rates);
        }
        "-h" | "--help" | "help" => {
            println!("usage: n12ctl [info | rates | rate HZ | defaults | set-rates RATE...]");
            println!();
            println!("  info              print the driver's state (default)");
            println!("  rates             print the advertised sample rates");
            println!("  rate HZ           change the sample rate (restarts the device)");
            println!("  defaults          offer the driver's default four rates");
            println!("  set-rates R...    offer a specific list, e.g. 44100 48000");
        }
        other => {
            eprintln!("unknown command: {}", other);
            std::process::exit(2);
        }
    }
}
