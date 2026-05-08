# aa-proxy-rs scripting

Guest WebAssembly scripting examples for [`aa-proxy-rs`](https://github.com/aa-proxy/aa-proxy-rs).

This repository contains a Rust guest component that implements the `aa:packet/packet-hook` WIT world used by `aa-proxy-rs`.

The generated `.wasm` component can be copied into the aa-proxy WASM hook directory and loaded by the host at runtime.

Example hook directory on device:

```bash
/data/wasm-hooks/
```

Only compiled `.wasm` component files should be placed there. Do not copy `.rs`, `.wit`, or source files into `/data/wasm-hooks/`.

---

## Features

A guest WASM component can:

- inspect Android Auto proxy packets
- forward or drop packets
- replace the currently processed packet
- send additional packets
- write logs through the host
- publish WebSocket events through the host
- receive script-level WebSocket events
- call selected aa-proxy REST endpoints synchronously
- call selected aa-proxy REST endpoints asynchronously
- keep guest-side state between calls
- run lifecycle hooks with `on-create` and `on-destroy`
- expose custom configuration fields in the aa-proxy config UI
- read custom configuration values through `host::get_config`
- receive live config updates through `on-config-changed`

---

## Runtime lifecycle

The host keeps one live WASM instance per loaded script.

That means guest-side state such as `thread_local!`, `Cell`, `RefCell`, static counters, cached config, and other in-memory values survive between calls.

Typical lifecycle:

```text
script file loaded
first script use or config discovery
  -> on_create()
  -> custom_configs()
modify_packet / ws_script_handler calls reuse the same guest instance
config changed from UI
  -> on_config_changed(name, value)
script file changed, removed, reloaded, or host shuts down
  -> on_destroy()
```

Reloading the script resets guest memory because the host creates a new component instance.

---

## Custom script configuration

Scripts can expose their own config section by implementing:

```rust
fn custom_configs() -> Vec<CustomConfigSection>
```

The host namespaces each config key automatically using the script file name.

For example, if the script file is:

```text
/data/wasm-hooks/test_hook.wasm
```

and the guest returns a config named:

```text
log_every
```

the UI/host key becomes:

```text
wasm.test_hook.log_every
```

Inside the guest, always read the local key only:

```rust
host::get_config("log_every")
```

Do not read the full namespaced key from the guest.

Custom config values are persisted by the host at:

```text
/data/aa-proxy-rs/wasm-config.toml
```

The `default_value` returned by `custom_configs()` is used when no saved value exists yet.

---

## Example custom config section

```rust
fn custom_configs() -> Vec<CustomConfigSection> {
    vec![CustomConfigSection {
        title: "WASM Config Test".to_string(),
        values: vec![
            CustomConfigEntry {
                name: "enabled".to_string(),
                typ: "bool".to_string(),
                description: "Enable packet logging from this WASM script".to_string(),
                default_value: "true".to_string(),
                values: None,
            },
            CustomConfigEntry {
                name: "log_every".to_string(),
                typ: "number".to_string(),
                description: "Log every N packets. Use 1 to log every packet.".to_string(),
                default_value: "20".to_string(),
                values: None,
            },
            CustomConfigEntry {
                name: "label".to_string(),
                typ: "string".to_string(),
                description: "Label printed in WASM info logs".to_string(),
                default_value: "wasm config test".to_string(),
                values: None,
            },
        ],
    }]
}
```

Supported `typ` values should match the aa-proxy config UI types, for example:

```text
bool
number
string
select
```

For select-like configs, set `values: Some(vec![...])`.

---

## Reading config from the guest

Use `host::get_config(name)`.

The return value is `Option<String>`.

Example helpers:

```rust
fn read_bool(name: &str, default: bool) -> bool {
    host::get_config(name)
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

fn read_u64(name: &str, default: u64) -> u64 {
    host::get_config(name)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_string(name: &str, default: &str) -> String {
    host::get_config(name).unwrap_or_else(|| default.to_string())
}
```

Recommended pattern:

- read and cache config in `on_create()`
- update the cached config in `on_config_changed()`
- use cached values inside high-frequency `modify_packet()` calls

Avoid parsing config repeatedly inside `modify_packet()` for every packet.

---

## Script limits

The host exposes WASM script limits in the normal aa-proxy config UI before the dynamic script config sections.

Typical fields:

```text
wasm_script_memory_limit_mb
wasm_script_instance_limit
wasm_script_memory_count_limit
wasm_script_table_limit
wasm_script_table_elements_limit
wasm_script_packet_epoch_deadline
wasm_script_lifecycle_epoch_deadline
```

These are host-side safety limits and are saved in the normal aa-proxy config, not in `/data/aa-proxy-rs/wasm-config.toml`.

`modify_packet()` should stay fast. If packet hooks start timing out, increase the packet epoch deadline carefully or move heavier work to `ws_script_handler()` / async REST calls.

---

## WIT interface

The guest component uses the `aa:packet/packet-hook` world.

```wit
package aa:packet;

interface types {
  enum proxy-type {
    head-unit,
    mobile-device,
  }

  record modify-context {
    sensor-channel: option<u8>,
    nav-channel: option<u8>,
    audio-channels: list<u8>,
  }

  record packet {
    proxy-type: proxy-type,
    channel: u8,
    packet-flags: u8,
    final-length: option<u32>,
    message-id: u16,
    payload: list<u8>,
  }

  record config-view {
    audio-max-unacked: u32,
    remove-tap-restriction: bool,
    video-in-motion: bool,
    developer-mode: bool,
    ev: bool,
    waze-lht-workaround: bool,
  }

  record custom-config-entry {
    name: string,
    typ: string,
    description: string,
    default-value: string,
    values: option<list<string>>,
  }

  record custom-config-section {
    title: string,
    values: list<custom-config-entry>,
  }

  enum decision {
    forward,
    drop,
  }
}

interface host {
  use types.{packet};

  replace-current: func(pkt: packet);
  send: func(pkt: packet);

  info: func(msg: string);
  error: func(msg: string);

  send-ws-event: func(topic: string, payload: string) -> bool;

  rest-call: func(method: string, path: string, body: string) -> string;
  rest-call-async: func(method: string, path: string, body: string) -> string;
  rest-result-topic: func() -> string;

  get-config: func(name: string) -> option<string>;
}

world packet-hook {
  use types.{
    modify-context,
    packet,
    config-view,
    custom-config-section,
    decision,
  };

  import host;

  export on-create: func();
  export on-destroy: func();

  export custom-configs: func() -> list<custom-config-section>;
  export on-config-changed: func(name: string, value: string);

  export modify-packet: func(ctx: modify-context, pkt: packet, cfg: config-view) -> decision;
  export ws-script-handler: func(topic: string, payload: string) -> string;
}
```

---

## Host functions

### `host::info(msg)`

Writes an info log through the aa-proxy host.

```rust
host::info("hello from wasm script");
```

### `host::error(msg)`

Writes an error log through the aa-proxy host.

```rust
host::error("something went wrong");
```

### `host::get_config(name)`

Returns the current saved value for a custom script config key.

Use the local key name, not the full `wasm.<script>.<key>` name.

```rust
let enabled = host::get_config("enabled")
    .and_then(|v| v.parse::<bool>().ok())
    .unwrap_or(true);
```

### `host::replace_current(pkt)`

Replaces the currently processed packet.

```rust
let mut modified = pkt.clone();
modified.payload = vec![0x12, 0x34, 0xAA, 0xBB];
host::replace_current(&modified);
Decision::Forward
```

### `host::send(pkt)`

Sends an additional packet.

```rust
let mut extra = pkt.clone();
extra.payload = vec![0x33, 0x33, 0x00, 0x01];
host::send(&extra);
Decision::Forward
```

### `host::send_ws_event(topic, payload)`

Publishes a WebSocket event through the aa-proxy host.

```rust
let ok = host::send_ws_event(
    "script.event",
    r#"{"message":"hello from wasm"}"#,
);

if !ok {
    host::error("failed to send websocket event");
}
```

### `host::rest_call(method, path, body)`

Calls an allowed aa-proxy REST endpoint synchronously.

Example:

```rust
let response = host::rest_call("GET", "/speed", "");
host::info(&format!("speed response: {response}"));
```

Use synchronous REST calls only for low-frequency actions. Avoid calling REST endpoints for every packet inside `modify_packet()`.

### `host::rest_call_async(method, path, body)`

Starts an allowed aa-proxy REST call in the host and immediately returns a request id.

The result is published later as a WebSocket event.

```rust
let request_id = host::rest_call_async(
    "POST",
    "/battery",
    r#"{"percentage":80}"#,
);

let result_topic = host::rest_result_topic();

host::info(&format!(
    "started async REST call request_id={request_id} result_topic={result_topic}"
));
```

### `host::rest_result_topic()`

Returns the WebSocket topic used by `rest_call_async()` results.

```rust
let topic = host::rest_result_topic();
host::info(&format!("async REST results will be published on: {topic}"));
```

---

## Guest exports

### `on_create()`

Called when the host creates the live guest instance.

Use it to initialize guest-side state and read initial custom config values.

```rust
fn on_create() {
    reload_config();
    host::info("script created");
}
```

### `on_destroy()`

Called before the live guest instance is destroyed during reload/removal/shutdown.

Use it to log final state or cleanup guest-side resources.

```rust
fn on_destroy() {
    host::info("script destroyed");
}
```

### `custom_configs() -> Vec<CustomConfigSection>`

Returns the custom config sections that should be appended to the aa-proxy config UI.

The host persists values and provides them back through `host::get_config()`.

### `on_config_changed(name, value)`

Called when one custom config value for this script changes.

`name` is the local key name, for example `log_every`, not `wasm.test_hook.log_every`.

```rust
fn on_config_changed(name: String, value: String) {
    reload_config();
    host::info(&format!("config changed: {name}={value}"));
}
```

### `modify_packet(ctx, pkt, cfg) -> Decision`

Called by the host for packet-processing hooks.

Return:

- `Decision::Forward` to forward the packet
- `Decision::Drop` to drop the packet

### `ws_script_handler(topic, payload) -> String`

Called by the host for script-level WebSocket events.

If the returned string is empty, the host may treat the event as not handled.

---

## Full config/logging test example

This example exposes three custom config values:

```text
enabled = true
log_every = 20
label = wasm config test
```

Set `log_every` to `1` from the aa-proxy config UI to log every packet.

```rust
#[allow(warnings)]
mod bindings;

use bindings::aa::packet::host;
use bindings::aa::packet::types::{
    ConfigView,
    CustomConfigEntry,
    CustomConfigSection,
    Decision,
    ModifyContext,
    Packet,
    ProxyType,
};
use bindings::Guest;

use std::cell::RefCell;

struct Component;

#[derive(Clone)]
struct RuntimeConfig {
    enabled: bool,
    log_every: u64,
    label: String,
    packet_count: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_every: 20,
            label: "wasm config test".to_string(),
            packet_count: 0,
        }
    }
}

thread_local! {
    static CONFIG: RefCell<RuntimeConfig> = RefCell::new(RuntimeConfig::default());
}

fn read_bool(name: &str, default: bool) -> bool {
    host::get_config(name)
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

fn read_u64(name: &str, default: u64) -> u64 {
    host::get_config(name)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_string(name: &str, default: &str) -> String {
    host::get_config(name).unwrap_or_else(|| default.to_string())
}

fn reload_config() {
    CONFIG.with(|cell| {
        let mut cfg = cell.borrow_mut();
        cfg.enabled = read_bool("enabled", true);
        cfg.log_every = read_u64("log_every", 20).max(1);
        cfg.label = read_string("label", "wasm config test");
    });
}

impl Guest for Component {
    fn custom_configs() -> Vec<CustomConfigSection> {
        vec![CustomConfigSection {
            title: "WASM Config Test".to_string(),
            values: vec![
                CustomConfigEntry {
                    name: "enabled".to_string(),
                    typ: "bool".to_string(),
                    description: "Enable packet logging from this WASM script".to_string(),
                    default_value: "true".to_string(),
                    values: None,
                },
                CustomConfigEntry {
                    name: "log_every".to_string(),
                    typ: "number".to_string(),
                    description: "Log every N packets. Use 1 to log every packet.".to_string(),
                    default_value: "20".to_string(),
                    values: None,
                },
                CustomConfigEntry {
                    name: "label".to_string(),
                    typ: "string".to_string(),
                    description: "Label printed in WASM info logs".to_string(),
                    default_value: "wasm config test".to_string(),
                    values: None,
                },
            ],
        }]
    }

    fn on_create() {
        reload_config();

        CONFIG.with(|cell| {
            let cfg = cell.borrow();
            host::info(&format!(
                "[wasm-config-test] on_create enabled={} log_every={} label={}",
                cfg.enabled, cfg.log_every, cfg.label
            ));
        });
    }

    fn on_destroy() {
        CONFIG.with(|cell| {
            let cfg = cell.borrow();
            host::info(&format!(
                "[wasm-config-test] on_destroy packet_count={}",
                cfg.packet_count
            ));
        });
    }

    fn on_config_changed(name: String, value: String) {
        reload_config();

        CONFIG.with(|cell| {
            let cfg = cell.borrow();
            host::info(&format!(
                "[wasm-config-test] on_config_changed {}={} -> enabled={} log_every={} label={}",
                name, value, cfg.enabled, cfg.log_every, cfg.label
            ));
        });
    }

    fn ws_script_handler(topic: String, payload: String) -> String {
        host::info(&format!(
            "[wasm-config-test] ws topic={} payload={}",
            topic, payload
        ));

        if topic == "script.get-speed" {
            return host::rest_call("GET", "/speed", "");
        }

        "".to_string()
    }

    fn modify_packet(_ctx: ModifyContext, pkt: Packet, cfg: ConfigView) -> Decision {
        CONFIG.with(|cell| {
            let mut rcfg = cell.borrow_mut();
            rcfg.packet_count += 1;

            if !rcfg.enabled {
                return;
            }

            if rcfg.packet_count % rcfg.log_every == 0 {
                let proxy = match pkt.proxy_type {
                    ProxyType::HeadUnit => "HeadUnit",
                    ProxyType::MobileDevice => "MobileDevice",
                };

                host::info(&format!(
                    "[wasm-config-test] packet_count={} label={} proxy={} channel={} message_id=0x{:04x} payload_len={} developer_mode={}",
                    rcfg.packet_count,
                    rcfg.label,
                    proxy,
                    pkt.channel,
                    pkt.message_id,
                    pkt.payload.len(),
                    cfg.developer_mode
                ));
            }
        });

        Decision::Forward
    }
}

bindings::export!(Component with_types_in bindings);
```

Expected logs after loading and saving config:

```text
[wasm-config-test] on_create enabled=true log_every=20 label=wasm config test
[wasm-config-test] on_config_changed log_every=1 -> enabled=true log_every=1 label=wasm config test
[wasm-config-test] packet_count=1 label=wasm config test proxy=MobileDevice channel=0 message_id=0x0001 payload_len=42 developer_mode=false
```

---

## WebSocket script-event examples

Example request:

```json
{
  "type": "script-event",
  "topic": "script.get-speed",
  "payload": ""
}
```

Example handler:

```rust
fn ws_script_handler(topic: String, payload: String) -> String {
    if topic == "script.get-speed" {
        return host::rest_call("GET", "/speed", "");
    }

    let _ = payload;
    "".to_string()
}
```

Example async REST handler:

```rust
fn ws_script_handler(topic: String, payload: String) -> String {
    if topic == "script.set-battery" {
        let request_id = host::rest_call_async("POST", "/battery", &payload);
        let result_topic = host::rest_result_topic();

        return format!(
            r#"{{"accepted":true,"requestId":"{}","resultTopic":"{}"}}"#,
            request_id,
            result_topic,
        );
    }

    "".to_string()
}
```

---

## Build

### 1. Install `cargo-component`

```bash
cargo install cargo-component --locked
```

### 2. Build the component

```bash
cargo component build --release
```

### 3. Find the built artifact

Depending on your `cargo-component` / target setup, the output is usually under one of these directories:

```bash
target/wasm32-wasip1/release/
target/wasm32-wasip2/release/
```

Example:

```bash
ls target/wasm32-wasip*/release/*.wasm
```

### 4. Copy the compiled `.wasm` into aa-proxy

```bash
cp target/wasm32-wasip*/release/aa_proxy_test_hook.wasm /data/wasm-hooks/test_hook.wasm
```

Restart aa-proxy or trigger script reload if needed.

---

## Debugging

### Check whether the script loaded

Look for logs like:

```text
[wasm] loaded wasm script: /data/wasm-hooks/test_hook.wasm
[wasm-config-test] on_create enabled=true log_every=20 label=wasm config test
```

### Check generated imports

If the host fails to instantiate the component because of missing WASI imports, inspect the component:

```bash
wasm-tools component wit target/wasm32-wasip*/release/aa_proxy_test_hook.wasm | grep wasi
```

or:

```bash
wasm-tools print target/wasm32-wasip*/release/aa_proxy_test_hook.wasm | grep "wasi:"
```

If the component imports `wasi:cli/environment@0.2.x`, the host must register WASI Preview 2 support in its component linker.

### Test config persistence

After changing custom config from the UI, check:

```bash
cat /data/aa-proxy-rs/wasm-config.toml
```

Expected shape:

```toml
[script.test_hook]
enabled = "true"
log_every = "1"
label = "wasm config test"
```

---

## Notes

- Use `host::info()` / `host::error()` instead of `println!()` / `eprintln!()`.
- Avoid file IO, OS environment reads, clocks, and random APIs unless the host provides the required WASI imports.
- Keep `modify_packet()` fast.
- Prefer `ws_script_handler()` for low-frequency REST calls and script-level events.
- Prefer `rest_call_async()` when the result does not need to be returned immediately.
- The same `.wasm` component can run on different host CPU architectures because WebAssembly component bytecode is architecture-independent.
- Guest state is runtime-only. Persist user settings through custom config, not guest memory.

---

## Suggested REST whitelist

The host should restrict script REST access to safe, low-risk endpoints.

Suggested allowed routes:

```text
POST /battery
POST /odometer
POST /tire-pressure
POST /inject_event
POST /inject_rotary
GET /speed
GET /battery-status
GET /odometer-status
GET /tire-pressure-status
```

Suggested blocked routes:

```text
/ws
/download
/restart
/reboot
/factory-reset
/upload-certs
/upload-hex-model
/userdata-backup
/userdata-restore
```

This keeps scripting useful while avoiding destructive or high-risk operations.
