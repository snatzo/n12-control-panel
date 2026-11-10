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

## The open question

The device type (`device+0xfc`) is set by `YamahaFWAudioDevice::setType`, and we
found **no caller** for it in the driver — so in our build path it appears to
stay 0, which forces the single-rate branch.

The vendor connection manager almost certainly set it (it made calls our agent
does not, e.g. `sRequestSetDevices`). Finding that call — or a way to trigger the
stream rebuild — is the remaining task before a sample-rate panel can work.

### Candidate next steps

1. Compare against the Mojave user-client capture: does the vendor manager call
   anything that ends in `setType`?
2. Try driving the rate through CoreAudio (`kAudioDevicePropertyNominalSampleRate`)
   and see whether it reaches `performFormatChange` / triggers a rebuild.
3. Check whether stopping and restarting the device (startChange/finishChange)
   after setting the list rebuilds the streams with all four rates — i.e. whether
   ordering alone is enough, without touching the type.

`n12ctl` is already the instrument for (2) and (3): it can set the list, and the
device's CoreAudio properties can be set from a tiny helper or Audio MIDI Setup.
