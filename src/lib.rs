#[allow(warnings)]
mod bindings;
pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

use bindings::Guest;
use bindings::aa::packet::host;
use bindings::aa::packet::types::{ModifyContext, ConfigView, Decision, Packet, ProxyType};

struct Component;

impl Guest for Component {
    fn ws_script_handler(topic: String, payload: String) -> String {
        host::info("ws event");
        if topic == "script.get-speed" {
            return host::rest_call("GET", "/speed", "");
        }
    
        let _ = payload;
    
        "".to_string()
    }

    fn modify_packet(ctx: ModifyContext, pkt: Packet, cfg: ConfigView) -> Decision {
        match pkt.proxy_type {
            ProxyType::HeadUnit => {
                host::info("packet came from HeadUnit");
            }
            ProxyType::MobileDevice => {
                host::info("packet came from MobileDevice");
            }
        }

        /*
        if pkt.message_id == 0x1234 {
            let mut p = pkt.clone();
            p.payload = vec![0x12, 0x34, 0xAA, 0xBB];
            host::replace_current(&p);
            return Decision::Forward;
        }

        if cfg.developer_mode && pkt.message_id == 0x2222 {
            let mut out = pkt.clone();
            out.payload = vec![0x33, 0x33, 0x00, 0x01];
            host::send(&out);
            return Decision::Forward;
        }

        if pkt.message_id == 0xDEAD {
            return Decision::Drop;
        }
        */

        let _ = cfg;
        Decision::Forward
    }
}

bindings::export!(Component with_types_in bindings);