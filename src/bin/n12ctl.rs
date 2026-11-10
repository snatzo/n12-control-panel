//! n12ctl - the command-line control tool for the Yamaha n-series on macOS.
//!
//! Run:
//!     n12ctl                 # print the driver's state
//!     n12ctl rates           # just the advertised sample rates
//!     n12ctl rate 48000      # change the sample rate (restarts the device)
//!     n12ctl set-rates 44100 48000 88200 96000
//!
//! Needs the Yamaha kexts loaded. `rate` also needs the n12-monterey checkout
//! (see `n12ctl::host`).

use n12ctl::host::rate_restart;
use n12ctl::n12::{fmt_kr, Device, DEFAULT_RATES};
use n12ctl::{coreaudio, iokit};

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

    // What CoreAudio thinks, which is what players and Audio MIDI Setup see.
    if let Some(id) = coreaudio::device_named("n12") {
        let name = coreaudio::device_name(id).unwrap_or_default();
        let rate = coreaudio::nominal_rate(id).unwrap_or(0.0);
        let ranges = coreaudio::available_rates(id);
        let list: Vec<String> = ranges
            .iter()
            .map(|r| {
                if r.min == r.max { format!("{}", r.min as u32) } else { format!("{}-{}", r.min as u32, r.max as u32) }
            })
            .collect();
        println!("  CoreAudio device \"{}\" (id {}): {} Hz; offers {} Hz",
                 name, id, rate as u32, list.join(", "));
    } else {
        println!("  CoreAudio: no n12 device (is the agent running?)");
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
        eprintln!("the driver refused. Valid rates are 32000, 44100, 48000, 88200, 96000 (max 8).");
    }
}

fn main() {
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
        "rate" => match args.get(1).and_then(|s| s.parse::<u32>().ok()) {
            Some(hz) => {
                println!("restarting the n12 at {} Hz ...", hz);
                match rate_restart(hz) {
                    Ok(()) => println!("done - the n12 should now be at {} Hz", hz),
                    Err(e) => eprintln!("failed: {}", e),
                }
            }
            None => {
                eprintln!("usage: n12ctl rate HZ     (e.g. n12ctl rate 48000)");
                std::process::exit(2);
            }
        },
        "set-rates" => {
            let rates: Vec<u32> = args.iter().skip(1).filter_map(|s| s.parse::<u32>().ok()).collect();
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
