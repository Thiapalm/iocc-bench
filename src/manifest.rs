//! JSON manifest answered to `iocc-bench --iocc-manifest`.
//!
//! The TUI calls this before anything else and without opening the device.

use crate::tools::{Direction, Pin, Port};

/// Flag the plugin answers to describe itself to the iocc TUI
pub const MANIFEST_FLAG: &str = "--iocc-manifest";

/// Panels and actions shown by `iocc tui` on the Plugins tab.
///
/// The views and the `on`/`off` actions declare `"device": "shared"`, so they
/// talk to the TUI broker (`IOCC_SOCKET`) and keep working while another plugin
/// uses the bus. The broker only exists on Unix; elsewhere the TUI releases the
/// MCP2221 before running the plugin (`exclusive`).
pub fn json() -> String {
    let device = if cfg!(unix) { "shared" } else { "exclusive" };
    serde_json::json!({
        "protocol": 1,
        "title": "Test bench (MCP23017)",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Ignition output on an MCP23017 expander through the MCP2221",
        "views": [
            {"id": "live", "label": "Ignition state", "kind": "metrics", "stream": true,
             "device": device, "command": ["watch", "--json", "--interval-ms", "500"]},
            {"id": "graph", "label": "Ignition graph", "kind": "chart", "metric": "Ignition",
             "min": 0, "max": 1, "stream": true, "device": device,
             "command": ["watch", "--json", "--interval-ms", "500"]},
            {"id": "status", "label": "Status", "kind": "table", "device": device,
             "refresh_ms": 2000, "command": ["status", "--json"]}
        ],
        "actions": [
            {"id": "on", "label": "Ignition ON",
             "description": "drive the configured pin high",
             "device": device, "command": ["on"]},
            {"id": "off", "label": "Ignition OFF",
             "description": "drive the configured pin low",
             "device": device, "command": ["off"]}
        ]
    })
    .to_string()
}

/// JSON table for the `status --json` view
#[allow(clippy::too_many_arguments)]
pub fn status_table(
    address: &str,
    transport: &str,
    port: Port,
    pin: Pin,
    direction: Direction,
    configured: Direction,
    ignition: bool,
    bus_info: &str,
) -> String {
    let rows = vec![
        vec!["Address".to_string(), address.to_string()],
        vec!["Transport".to_string(), transport.to_string()],
        vec!["Port".to_string(), port.to_string()],
        vec!["Pin".to_string(), pin.to_string()],
        vec!["Direction (chip)".to_string(), direction.to_string()],
        vec!["Direction (config)".to_string(), configured.to_string()],
        vec![
            "Ignition".to_string(),
            if ignition { "ON" } else { "OFF" }.to_string(),
        ],
        vec!["Bus".to_string(), bus_info.replace('\n', " ")],
    ];
    serde_json::json!({
        "columns": ["Field", "Value"],
        "rows": rows,
    })
    .to_string()
}

/// JSON metrics for the `watch --json` stream
pub fn metrics(ignition: bool) -> String {
    serde_json::json!({
        "metrics": [
            {"name": "Ignition", "value": if ignition { 1.0 } else { 0.0 }, "unit": ""},
        ]
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid_for_the_tui_protocol() {
        let value: serde_json::Value = serde_json::from_str(&json()).unwrap();
        assert_eq!(value["protocol"], 1);
        assert_eq!(value["views"].as_array().unwrap().len(), 3);
        assert_eq!(value["actions"].as_array().unwrap().len(), 2);

        let expected = if cfg!(unix) { "shared" } else { "exclusive" };
        let views = value["views"].as_array().unwrap();
        for view in views {
            assert_eq!(view["device"], expected);
        }
        let live = views.iter().find(|view| view["id"] == "live").unwrap();
        assert_eq!(live["kind"], "metrics");
        assert_eq!(live["stream"].as_bool(), Some(true));
        let graph = views.iter().find(|view| view["id"] == "graph").unwrap();
        assert_eq!(graph["kind"], "chart");
        assert_eq!(graph["metric"], "Ignition");
        assert_eq!(graph["min"], 0);
        assert_eq!(graph["max"], 1);
        let status = views.iter().find(|view| view["id"] == "status").unwrap();
        assert_eq!(status["kind"], "table");
        assert_eq!(status["refresh_ms"], 2000);

        let actions = value["actions"].as_array().unwrap();
        assert_eq!(actions[0]["id"], "on");
        assert!(actions[0].get("confirm").is_none(), "on must not confirm");
        assert_eq!(actions[1]["id"], "off");
    }

    #[test]
    fn status_and_metrics_parse() {
        let table: serde_json::Value = serde_json::from_str(&status_table(
            "0x20",
            "broker (shared with the TUI)",
            Port::Portb,
            Pin::Pin1,
            Direction::Output,
            Direction::Input,
            true,
            "MCP rev A\nfirmware 1.0",
        ))
        .unwrap();
        assert_eq!(table["columns"][0], "Field");
        assert_eq!(table["rows"][2][1], "portb");
        assert_eq!(table["rows"][3][1], "pin1");
        assert_eq!(table["rows"][4][1], "output");
        assert_eq!(table["rows"][5][1], "input");
        assert_eq!(table["rows"][6][1], "ON");
        assert_eq!(table["rows"][7][1], "MCP rev A firmware 1.0");

        let on: serde_json::Value = serde_json::from_str(&metrics(true)).unwrap();
        assert_eq!(on["metrics"][0]["name"], "Ignition");
        assert_eq!(on["metrics"][0]["value"], 1.0);
        let off: serde_json::Value = serde_json::from_str(&metrics(false)).unwrap();
        assert_eq!(off["metrics"][0]["value"], 0.0);
    }
}
