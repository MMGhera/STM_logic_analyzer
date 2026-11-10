# logic_analyzer_rust

A 16-channel logic analyzer for the **STM32H7S3L8**, written in embedded Rust with [Embassy](https://embassy.dev). It uses the SUMP/Openbench Logic Sniffer protocol over USB High-Speed CDC, so it works with **PulseView** and **sigrok-cli**.

## Features

- 16 channels on **PE0–PE15**. Channel N is PEN. Inputs have pull-downs.
- Up to 16384 samples per capture.
- Sample rates are set by the SUMP divider: 100 MHz / (div + 1). Up to 20 MHz is currently advertised, not tested yet.
- USB HS CDC-ACM, so it shows up as `/dev/ttyACMx` and needs no drivers.

## How it works

Capture is a polled loop. The CPU reads `GPIOE->IDR` and paces each read against the DWT cycle counter, using absolute deadlines so the sample timing doesn't drift. At 400 MHz, one SUMP tick (10 ns) is 4 CPU cycles, so divider `d` means one sample every `4·(d+1)` cycles. The samples are reversed and sent newest-first, as SUMP requires.

```
src/
├── main.rs     # clocks, USB HS setup, serial loop
├── sump.rs     # SUMP command parser + metadata
└── capture.rs  # GPIOE setup, DWT-timed capture loop
```

## Build & flash

You need the `thumbv7em-none-eabihf` target and [probe-rs](https://probe.rs).

```sh
rustup target add thumbv7em-none-eabihf
cargo install probe-rs-tools --locked
DEFMT_LOG=info cargo run --release
```

Flash through the ST-LINK port, then connect the USB HS port (CN2) to the PC.

## Usage

**PulseView:** choose Connect to Device, then the **Openbench Logic Sniffer & SUMP compatibles (ols)** driver, then Serial Port `/dev/ttyACM0`, then Scan.

**sigrok-cli:**
```sh
sigrok-cli -d ols:conn=/dev/ttyACM0 --config samplerate=1m --samples 1000 -O bits
```

## Limitations / TODO

- No hardware triggers yet. Every capture starts immediately.
- At least one of channels 8–15 must stay enabled, because the firmware always sends 2 bytes per sample.
- The maximum rate is not yet verified. The loop speed depends on GPIO bus latency, so it still needs to be measured against a known signal.
- The polled capture blocks USB while it runs.
- Planned: timer-triggered DMA capture, triggers, larger buffer.
