# `stm32f103` playground

[`probe-run`] + [`defmt`] + [`flip-link`] + [`rtic`] Rust embedded playground

[`probe-run`]: https://crates.io/crates/probe-run
[`defmt`]: https://github.com/knurling-rs/defmt
[`flip-link`]: https://github.com/knurling-rs/flip-link
[`rtic`]: https://github.com/rtic-rs/cortex-m-rtic

## Dependencies

#### 1. `flip-link`:

```console
$ cargo install flip-link
```

#### 2. `probe-run`:

```console
$ cargo install probe-run
```

## Run!

Start by `cargo run`-ning `src/bin/serial.rs`:

```console
$ # `rb` is an alias for `run --bin`
$ cargo rb serial
  Finished dev [optimized + debuginfo] target(s) in 0.3s
  Running `probe-run --chip STM32F103VC target/thumbv7-none-eabi/debug/serial`
  (HOST) INFO  flashing program (13.39 KiB)
  (HOST) INFO  success!
───────────────────────────────────────────────────────────────────────────────
```

## midi_raw

Emitting a simple sequence of note on/off messages.

``` console
DEFMT_LOG=trace cargo rrb midi_raw
...
INFO  init
TRACE note on
TRACE note off
TRACE note on
...
```

Notice, the linux midi driver will block after first message if no listener attached.

Useful commands in Linux to view a midi stream:

``` console
> lsusb 
...
Bus 001 Device 024: ID 16c0:27de Van Ooijen Technische Informatica MIDI class devices
...

> amidi -l
...
IO  hw:4,0,0  MIDI Device MIDI 1
...

> aconnect -i
...
client 32: 'MIDI Device' [type=kernel,card=4]
    0 'MIDI Device MIDI 1'
...

> aseqdump -p 32
...
Waiting for data. Press Ctrl+C to end.
Source  Event                  Ch  Data
  0:1   Port subscribed            143:0 -> 128:0
 32:0   Note on                 0, note 72, velocity 64
...
