#[allow(warnings)]
mod bindings;

pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

use std::cell::{Cell, RefCell};

use bindings::aa::packet::host;
use bindings::aa::packet::types::{
    ConfigView, CustomConfigEntry, CustomConfigSection, Decision, ModifyContext, Packet, ProxyType,
};
use bindings::Guest;

struct Component;

#[derive(Clone, Debug)]
struct RuntimeConfig {
    enabled: bool,
    log_every: u64,
    label: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_every: 20,
            label: "wasm config test".to_string(),
        }
    }
}

thread_local! {
    static CONFIG: RefCell<RuntimeConfig> = RefCell::new(RuntimeConfig::default());
    static PACKET_COUNT: Cell<u64> = const { Cell::new(0) };
}

fn parse_bool(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

fn read_bool(name: &str, default: bool) -> bool {
    host::get_config(name)
        .map(|value| parse_bool(&value, default))
        .unwrap_or(default)
}

fn read_u64(name: &str, default: u64) -> u64 {
    host::get_config(name)
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_string(name: &str, default: &str) -> String {
    host::get_config(name).unwrap_or_else(|| default.to_string())
}

fn load_config_from_host() -> RuntimeConfig {
    let default = RuntimeConfig::default();

    RuntimeConfig {
        enabled: read_bool("enabled", default.enabled),
        log_every: read_u64("log_every", default.log_every),
        label: read_string("label", &default.label),
    }
}

fn set_config(new_config: RuntimeConfig) {
    CONFIG.with(|config| {
        *config.borrow_mut() = new_config;
    });
}

fn with_config<R>(f: impl FnOnce(&RuntimeConfig) -> R) -> R {
    CONFIG.with(|config| f(&config.borrow()))
}

fn local_config_name(name: &str) -> &str {
    // The host should call on-config-changed with the local name, such as "enabled".
    // This fallback also tolerates a full UI key like "wasm.config_test.enabled".
    name.rsplit('.').next().unwrap_or(name)
}

fn proxy_type_name(proxy_type: ProxyType) -> &'static str {
    match proxy_type {
        ProxyType::HeadUnit => "HeadUnit",
        ProxyType::MobileDevice => "MobileDevice",
    }
}

impl Guest for Component {
    fn on_create() {
        let config = load_config_from_host();
        set_config(config.clone());

        host::info(&format!(
            "[wasm-config-test] on_create enabled={} log_every={} label={}",
            config.enabled, config.log_every, config.label,
        ));
    }

    fn on_destroy() {
        let count = PACKET_COUNT.with(|counter| counter.get());
        host::info(&format!(
            "[wasm-config-test] on_destroy packet_count={}",
            count,
        ));
    }

    fn custom_configs() -> Vec<CustomConfigSection> {
        vec![CustomConfigSection {
            title: "WASM Config Test".to_string(),
            values: vec![
                CustomConfigEntry {
                    name: "enabled".to_string(),
                    typ: "boolean".to_string(),
                    description: "Enable info logs from this WASM test script.".to_string(),
                    default_value: "true".to_string(),
                    values: None,
                },
                CustomConfigEntry {
                    name: "log_every".to_string(),
                    typ: "integer".to_string(),
                    description: "Write one info log after this many packets. Use 0 to disable packet logs.".to_string(),
                    default_value: "20".to_string(),
                    values: None,
                },
                CustomConfigEntry {
                    name: "label".to_string(),
                    typ: "string".to_string(),
                    description: "Label included in the test info log.".to_string(),
                    default_value: "wasm config test".to_string(),
                    values: None,
                },
            ],
        }]
    }

    fn on_config_changed(name: String, value: String) {
        let local_name = local_config_name(&name).to_string();

        CONFIG.with(|config| {
            let mut config = config.borrow_mut();

            match local_name.as_str() {
                "enabled" => {
                    config.enabled = parse_bool(&value, config.enabled);
                }
                "log_every" => {
                    if let Ok(parsed) = value.trim().parse::<u64>() {
                        config.log_every = parsed;
                    } else {
                        host::error(&format!(
                            "[wasm-config-test] invalid log_every value: {}",
                            value,
                        ));
                    }
                }
                "label" => {
                    config.label = value.clone();
                }
                _ => {
                    host::error(&format!(
                        "[wasm-config-test] unknown config changed: {}={}",
                        name, value,
                    ));
                }
            }

            host::info(&format!(
                "[wasm-config-test] on_config_changed {}={} -> enabled={} log_every={} label={}",
                name, value, config.enabled, config.log_every, config.label,
            ));
        });
    }

    fn modify_packet(ctx: ModifyContext, pkt: Packet, cfg: ConfigView) -> Decision {
        let packet_count = PACKET_COUNT.with(|counter| {
            let next = counter.get().saturating_add(1);
            counter.set(next);
            next
        });

        let should_log = with_config(|config| {
            config.enabled && config.log_every > 0 && packet_count % config.log_every == 0
        });

        if should_log {
            with_config(|config| {
                host::info(&format!(
                    "[wasm-config-test] packet_count={} label={} proxy={} channel={} message_id=0x{:04X} payload_len={} developer_mode={}",
                    packet_count,
                    config.label,
                    proxy_type_name(pkt.proxy_type),
                    pkt.channel,
                    pkt.message_id,
                    pkt.payload.len(),
                    cfg.developer_mode,
                ));
            });
        }

        let _ = ctx;

        Decision::Forward
    }

    fn ws_script_handler(topic: String, payload: String) -> String {
        host::info(&format!(
            "[wasm-config-test] ws_script_handler topic={} payload={}",
            topic, payload,
        ));

        if topic == "script.config-test" {
            let packet_count = PACKET_COUNT.with(|counter| counter.get());

            return with_config(|config| {
                format!(
                    r#"{{"enabled":{},"logEvery":{},"label":"{}","packetCount":{}}}"#,
                    config.enabled,
                    config.log_every,
                    config.label.replace('"', "\\\""),
                    packet_count,
                )
            });
        }
        if topic == "script.get-speed" {
            return host::rest_call("GET", "/speed", "");
        }

        "".to_string()
    }
}

bindings::export!(Component with_types_in bindings);
