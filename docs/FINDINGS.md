# Findings: how the n12's sample rate is (and isn't) set

Notes from reading the installed Yamaha audio driver for the purpose of adding a
sample-rate control. No Yamaha code is reproduced; this describes the interface
we observed.

## The driver's rate model (from its own code)

Two lists, on two different objects:

* **`YamahaFWStreamDevice`** (the top-level device) keeps a list at
  `streamDevice+0x12c` with its length at `+0x128`. This is what
  `sGetAvailableSampleRates` (selector 82) reads and what
  `sSetAvailableSampleRates` (selector 76) writes. Its compiled-in default is
  `s_availableSampleRatesDefault = {44100, 48000, 88200, 96000}`.
* **`YamahaFWAudioEngine`** keeps its own copy at `engine+0x148` / `+0x144`,
  used when the streams are built.

`YamahaFWStreamDevice::setAvailableSampleRates` (selector 76) writes the first
list and then, for each engine where a type check returns 1, calls
`YamahaFWAudioEngine::setAvailableSampleRate` to update the engine's copy.

## Where the advertised rates come from

`YamahaFWAudioEngine::createNewAudioStream` is what actually publishes a rate to
CoreAudio: it calls `IOAudioStream::addAvailableFormat(format, minRate, maxRate,
…)` for each rate the stream supports. It does that in one of two ways:

```
if (engine+0x1c8 == 1)                  // the device "type"
    for each rate in the engine list    // 0x144 count, 0x148 array
        addAvailableFormat(rate)
else
    addAvailableFormat(the single initial rate)   // 44100
```

`engine+0x1c8` is written **only** by `YamahaFWAudioEngine::setType`, which is
called from `YamahaFWAudioDevice::createAudioEngine` with the value
`device+0xfc`. So the whole multi-rate behaviour hinges on the device's
**type** being `1`.

## What we measured

`n12ctl info` (on the live device, kexts loaded) reports:

```
audio devices          : 1
available sample rates : 44100, 48000, 88200, 96000 Hz   <- the driver KNOWS all four
FireWire speed         : 3 (S300)
```

…while CoreAudio still advertises **44100 only**. So:

* the **list** plumbing works (selector 76 writes it, selector 82 reads it back);
* the **advertisement** does not, because the streams were built with the
  single-rate branch.

## Solved: the device "type" is a field in `sCreateAudioDevice`

The 48-byte `sCreateAudioDevice` structure's field at **`+0x0c`** is not an index —
it is the **device type**, and `createAudioDeviceFunction` feeds it straight to
`YamahaFWAudioDevice::setType` (which writes `device+0xfc`). Mapping the other
fields through the same code path gives the whole structure:

| struct field | device setter |
|---|---|
| `+0x00` guid | `setGUID` |
| `+0x08` field08 | `setVendorID` |
| **`+0x0c` type** | **`setType`** |
| `+0x10` name | `setDeviceNameStr` |
| `+0x20` vendor | `setVendorNameStr` |

We had been sending type **0**. With **1**, the engine's type check passes and
the multi-rate branch in `createNewAudioStream` runs. Measured on the live device
(`n12-monterey`'s agent, `--dev-type=N`):

```
--dev-type=0 :  available: 44100..44100
--dev-type=1 :  available: 44100..44100 48000..48000 88200..88200 96000..96000
```

`n12-monterey`'s agent now defaults to type 1, so CoreAudio advertises all four
rates.

## Still open: actually switching the rate

Setting CoreAudio's `kAudioDevicePropertyNominalSampleRate` returns success but
the rate stays put:

```
set 48000 Hz -> status 0 ; now 44100 Hz
```

So the property alone is not enough — the driver's own rate-change path has to be
driven (its `performFormatChange` only acts inside a change cycle, and the desk's
own clock/rate is involved). That, and making the desk follow, is the next task.


`n12ctl` is already the instrument for (2) and (3): it can set the list, and the
device's CoreAudio properties can be set from a tiny helper or Audio MIDI Setup.
