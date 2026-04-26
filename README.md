# aa-proxy-rs scripting

Guest WebAssembly scripting examples for `aa-proxy-rs`.

This repository contains a minimal Rust guest component that implements the
`aa:packet/packet-hook` WIT world used by `aa-proxy-rs`.

The generated `.wasm` component can be copied into the aa-proxy WASM hooks
directory and loaded by the host at runtime.

Example hook directory on device:

```bash
/data/wasm-hooks/
```

Only compiled `.wasm` component files should be placed there.

---

## Features

This scripting interface allows a guest WASM component to:

- inspect Android Auto proxy packets
- decide whether to forward or drop packets
- replace the currently processed packet
- send additional packets
- write logs through the host
- publish WebSocket events through the host
- receive script-level WebSocket events
- call selected aa-proxy REST endpoints synchronously
- call selected aa-proxy REST endpoints asynchronously

---

## WIT interface

The guest component uses this WIT world:

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
}

world packet-hook {
  use types.{modify-context, packet, config-view, decision};

  import host;

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

The host forwards the call to the aa-proxy REST server running on:

```text
http://127.0.0.1:80
```

Example GET:

```rust
let response = host::rest_call(
    "GET",
    "/speed",
    "",
);

host::info(&format!("speed response: {response}"));
```

Example POST:

```rust
let response = host::rest_call(
    "POST",
    "/battery",
    r#"{"percentage":80}"#,
);

host::info(&format!("battery response: {response}"));
```

The returned value is a JSON string. A typical response shape is:

```json
{
  "ok": true,
  "status": 200,
  "body": "..."
}
```

Use synchronous REST calls only for low-frequency script actions. Avoid calling
REST endpoints for every packet inside `modify_packet`, especially for video,
audio, or high-frequency sensor packets.

### `host::rest_call_async(method, path, body)`

Starts an allowed aa-proxy REST call in the host and immediately returns a UUID
request id.

The result is published later as a WebSocket event.

```rust
let request_id = host::rest_call_async(
    "POST",
    "/battery",
    r#"{"percentage":80}"#,
);

host::info(&format!("started async REST call: {request_id}"));
```

The result topic can be queried with:

```rust
let result_topic = host::rest_result_topic();
```

By default this is expected to be:

```text
script.rest.result
```

Example async result payload:

```json
{
  "requestId": "5ec4e020-f0be-49fd-9087-5dfac090bb5d",
  "method": "POST",
  "path": "/battery",
  "result": "{\"ok\":true,\"status\":200,\"body\":\"...\"}"
}
```

### `host::rest_result_topic()`

Returns the WebSocket topic used by `rest_call_async`.

```rust
let topic = host::rest_result_topic();

host::info(&format!("async REST results will be published on: {topic}"));
```

---

## Guest exports

### `modify_packet(ctx, pkt, cfg) -> Decision`

Called by the host for packet-processing hooks.

Return:

- `Decision::Forward` to forward the packet
- `Decision::Drop` to drop the packet

Example:

```rust
fn modify_packet(ctx: ModifyContext, pkt: Packet, cfg: ConfigView) -> Decision {
    match pkt.proxy_type {
        ProxyType::HeadUnit => {
            host::info("packet came from HeadUnit");
        }
        ProxyType::MobileDevice => {
            host::info("packet came from MobileDevice");
        }
    }

    if pkt.message_id == 0xDEAD {
        host::info("dropping packet 0xDEAD");
        return Decision::Drop;
    }

    let _ = ctx;
    let _ = cfg;

    Decision::Forward
}
```

### `ws_script_handler(topic, payload) -> String`

Called by the host for script-level WebSocket events.

If the returned string is empty, the host may treat the event as not handled.
If the returned string is non-empty, the host can publish it as a replacement
payload for the same topic.

Example:

```rust
fn ws_script_handler(topic: String, payload: String) -> String {
    host::info(&format!("ws event received: topic={topic}, payload={payload}"));

    if topic == "script.ping" {
        return r#"{"pong":true}"#.to_string();
    }

    "".to_string()
}
```

---

## Full Rust example

```rust
#[allow(warnings)]
mod bindings;

use bindings::Guest;
use bindings::aa::packet::host;
use bindings::aa::packet::types::{
    ConfigView,
    Decision,
    ModifyContext,
    Packet,
    ProxyType,
};

struct Component;

impl Guest for Component {
    fn modify_packet(ctx: ModifyContext, pkt: Packet, cfg: ConfigView) -> Decision {
        match pkt.proxy_type {
            ProxyType::HeadUnit => {
                host::info("packet came from HeadUnit");
            }
            ProxyType::MobileDevice => {
                host::info("packet came from MobileDevice");
            }
        }

        if cfg.developer_mode && pkt.message_id == 0x2222 {
            let mut extra = pkt.clone();
            extra.payload = vec![0x33, 0x33, 0x00, 0x01];

            host::send(&extra);

            return Decision::Forward;
        }

        if pkt.message_id == 0xDEAD {
            host::info("dropping packet 0xDEAD");
            return Decision::Drop;
        }

        let _ = ctx;

        Decision::Forward
    }

    fn ws_script_handler(topic: String, payload: String) -> String {
        host::info(&format!("ws event received: topic={topic}, payload={payload}"));

        if topic == "script.ping" {
            return r#"{"pong":true}"#.to_string();
        }

        if topic == "script.speed" {
            return host::rest_call("GET", "/speed", "");
        }

        if topic == "script.battery" {
            let request_id = host::rest_call_async(
                "POST",
                "/battery",
                &payload,
            );

            let result_topic = host::rest_result_topic();

            return format!(
                r#"{{"accepted":true,"requestId":"{}","resultTopic":"{}"}}"#,
                request_id,
                result_topic,
            );
        }

        if topic == "script.custom-event" {
            let ok = host::send_ws_event(
                "script.event",
                r#"{"message":"custom event from wasm"}"#,
            );

            return format!(r#"{{"sent":{}}}"#, ok);
        }

        "".to_string()
    }
}

bindings::export!(Component with_types_in bindings);
```

---

## Example: synchronous REST call from WebSocket event

```rust
fn ws_script_handler(topic: String, payload: String) -> String {
    if topic == "script.get-speed" {
        return host::rest_call("GET", "/speed", "");
    }

    let _ = payload;

    "".to_string()
}
```

A client can send:

```json
{
  "type": "script-event",
  "topic": "script.get-speed",
  "payload": ""
}
```

The script calls `/speed` and returns the response immediately.

---

## Example: asynchronous REST call from WebSocket event

```rust
fn ws_script_handler(topic: String, payload: String) -> String {
    if topic == "script.set-battery" {
        let request_id = host::rest_call_async(
            "POST",
            "/battery",
            &payload,
        );

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

A client can send:

```json
{
  "type": "script-event",
  "topic": "script.set-battery",
  "payload": "{\"percentage\":80}"
}
```

The immediate response contains the UUID request id:

```json
{
  "accepted": true,
  "requestId": "5ec4e020-f0be-49fd-9087-5dfac090bb5d",
  "resultTopic": "script.rest.result"
}
```

The actual REST result is later published on:

```text
script.rest.result
```

---

## Example: publish a WebSocket event from script

```rust
fn ws_script_handler(topic: String, payload: String) -> String {
    if topic == "script.emit" {
        let ok = host::send_ws_event(
            "script.output",
            r#"{"hello":"from wasm"}"#,
        );

        return format!(r#"{{"sent":{}}}"#, ok);
    }

    let _ = payload;

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

### 3. Copy the built artifact into aa-proxy

The output file name is usually under:

```bash
target/wasm32-wasip1/release/
```

Example:

```bash
cp target/wasm32-wasip1/release/aa_proxy_test_hook.wasm /data/wasm-hooks/10_test_hook.wasm
```

---

## Notes

- Do not put `.rs` files into `/data/wasm-hooks/`.
- `/data/wasm-hooks/` should contain compiled `.wasm` component files only.
- Edit source here, rebuild, then copy the compiled `.wasm` output.
- Avoid expensive or blocking work inside high-frequency `modify_packet` calls.
- Prefer `ws_script_handler` for REST calls and script-level events.
- Prefer `rest_call_async` when the result does not need to be returned immediately.
- Use `rest_call` only when the result is needed immediately.
- The same `.wasm` component can run on different host CPU architectures because WebAssembly component bytecode is architecture-independent.

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
GET  /speed
GET  /battery-status
GET  /odometer-status
GET  /tire-pressure-status
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