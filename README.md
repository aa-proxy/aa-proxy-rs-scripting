# aa-proxy Wasmtime component sample

This is a minimal guest component project for the `aa:packet/packet-hook` WIT world.

## What it does

This project is a minimal Wasmtime guest component for `aa-proxy-rs`.

It demonstrates how to:

- implement the `aa:packet/packet-hook` WIT world
- receive packet data from the host as typed WIT records
- inspect packet metadata such as proxy type, channel, flags, message id, and payload
- read selected host-side context/config values passed into the guest
- return a decision such as `forward` or `drop`
- call host functions to:
  - replace the current packet
  - send a new packet
  - write host-side log messages

The component is meant to be compiled into a `.wasm` WebAssembly component and then loaded by `aa-proxy-rs` from the hook directory.

In short, this repo is a starting point for writing packet-processing hooks for `aa-proxy-rs` in Rust, compiled to Wasmtime components.

## Build

### 1) Install cargo-component

```bash
cargo install cargo-component --locked
```

### 2) Build the component

```bash
cargo component build --release
```

### 3) Copy the built artifact into aa-proxy

The output file name is typically:

```bash
target/wasm32-wasip1/release/aa_proxy_test_hook.wasm
```

Copy it to your hook folder, for example:

```bash
cp target/wasm32-wasip1/release/aa_proxy_test_hook.wasm /data/wasm-hooks/10_test_hook.wasm
```

## Why the same `.wasm` works on ARM and RISC-V

The guest artifact is WebAssembly component bytecode, which is architecture-independent.
Wasmtime compiles that same `.wasm` for the current host CPU at runtime.
So you normally build the hook once and reuse the same `.wasm` on x86_64, ARM64, or riscv64 hosts, as long as the host Wasmtime build supports that architecture.

## Notes

- Do not put `.rs` files into `/data/wasm-hooks/`.
- `/data/wasm-hooks/` should contain compiled `.wasm` component files only.
- Edit source here, then rebuild and copy the `.wasm` output.
