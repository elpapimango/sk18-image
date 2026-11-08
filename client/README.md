# On-device client (Rust)

Runs on the SK-18 itself. For now one crate:

- `sk18-keys`: decodes the key controller's UART1 stream (frames of 13 bytes at 115200 baud, see
  `docs/uart1-key-protocol.md`), maps the firmware's key numbering to what the user sees for a given rotation,
  and prints the events. Library plus a small command line tool.

```
cargo test                                   # unit tests and the real capture in tests/data
cargo clippy --all-targets -- -D warnings
cargo check --target armv7-unknown-linux-gnueabihf   # the board is a 32-bit hard-float Cortex-A7, glibc
cargo run -- --file sk18-keys/tests/data/capture-115200.hex   # decode the capture
cargo run -- --port /dev/ttyS0 --orientation 90               # on the board: live events
```

The serial port code is Unix only; the decoder and the orientation maths build and test everywhere.
Dependencies are kept to a minimum (`rustix` for the serial line settings) and `Cargo.lock` is committed.
