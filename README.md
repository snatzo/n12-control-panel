# n12ctl — a control panel for the Yamaha n-series on macOS

A small, dependency-free Rust tool to read and (soon) change the settings of a
**Yamaha n-series mixer (n12/n8)** on a modern Mac — sample rate, clock source,
master clock, sync status.

It is the companion to **[n12-monterey](https://github.com/snatzo/n12-monterey)**,
which makes the desk appear as a CoreAudio device. The old,
"Yamaha Steinberg FW Control Panel" that used to show these settings is 32-bit
and does not run on current macOS; this is a 64-bit replacement.

## Status

**Milestone 1 (done): read the driver's state.**

```
$ n12ctl info
Yamaha n-series driver
  audio devices              : 1
  current sample rate        : 0 Hz
  available sample rates     : 44100, 48000, 88200, 96000 Hz
  clock-sync channel (SYT)   : slaved to isoch channel 16
  FireWire speed             : 3 (S300)
```

**Milestone 2 (done): the sample rate can be changed.** The device advertises
44.1/48/88.2/96 kHz (once the agent sends device type 1 — see
[docs/FINDINGS.md](docs/FINDINGS.md)), and `n12ctl rate HZ` switches it. The
driver only applies a rate when the device starts, so a change restarts the
device; that is what `n12ctl rate` orchestrates.

**Milestone 3 (planned): the panel itself** — a menu-bar item or small window.

## Requirements

* macOS with the **Yamaha Steinberg FW Driver** installed **and its kexts
  loaded**. `n12ctl` talks to the kexts, not to the desk directly; without them
  it will say it cannot open `YamahaFWStreamDevice`.
* Rust (any recent stable) — `cargo build` needs no network, because there are
  **no external crates**.

## Build and run

```sh
cd n12_control_panel
cargo build --release
./target/release/n12ctl            # or: cargo run -- info
```

```
n12ctl                 print the driver's state (same as `info`)
n12ctl rates           print the advertised sample rates
n12ctl rate 48000      change the sample rate (restarts the device)
n12ctl defaults        offer the driver's default four rates
n12ctl set-rates 44100 48000 88200 96000
```

## How it works

The Yamaha driver publishes a service called `YamahaFWStreamDevice`. A user-space
program finds it with IOKit, opens a *user client*, and calls numbered methods on
it. `src/iokit.rs` is the smallest FFI to do that (find, open, and the two
`IOConnectCall…` shapes); `src/n12.rs` names the method numbers and turns raw
`u64`s into friendlier shapes. It is the same interface the C tools in
`n12-monterey` use, rewritten in Rust.

## Licence

Apache-2.0.
