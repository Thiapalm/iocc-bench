//! JSON manifest answered to `iocc-bench --iocc-manifest`.
//!
//! The TUI calls this before anything else and without opening the device.

use crate::config::Pinconf;
use crate::tools::Direction;

/// Flag the plugin answers to describe itself to the iocc TUI
pub const MANIFEST_FLAG: &str = "--iocc-manifest";

/// Panels and actions shown by `iocc tui` on the Plugins tab.
///
/// The views and the actions declare `"device": "shared"`, so they talk to the
/// TUI broker (`IOCC_SOCKET`) and keep working while another plugin uses the
/// bus. The broker only exists on Unix; elsewhere the TUI releases the MCP2221
/// before running the plugin (`exclusive`). The *Configure ignition pin* action
/// defaults come from the saved configuration and are written back when it runs.
pub fn json(ignition: &Pinconf) -> String {
    let device = if cfg!(unix) { "shared" } else { "exclusive" };
    let pin_options: Vec<String> = (0..=7).map(|pin| format!("pin{pin}")).collect();
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
             "description": "turn the ignition ON (physical level follows the configured logic)",
             "device": device, "command": ["on"]},
            {"id": "off", "label": "Ignition OFF",
             "description": "turn the ignition OFF (physical level follows the configured logic)",
             "device": device, "command": ["off"]},
            {"id": "configure", "label": "Configure ignition pin",
             "description": "change the port, pin, direction or inverted logic and save them in config.yaml",
             "args": [
                {"name": "port", "label": "Port", "type": "choice",
                 "options": ["porta", "portb"], "default": ignition.port.to_string()},
                {"name": "pin", "label": "Pin", "type": "choice",
                 "options": pin_options, "default": ignition.pin.to_string()},
                {"name": "direction", "label": "Direction", "type": "choice",
                 "options": ["input", "output"], "default": ignition.direction.to_string()},
                {"name": "inverted", "label": "Inverted logic (normally closed relay)",
                 "type": "choice", "options": ["false", "true"],
                 "default": ignition.inverted.to_string()}
             ],
             "device": device,
             "command": ["set", "--port", "{port}", "--pin", "{pin}", "--direction", "{direction}",
                         "--inverted", "{inverted}"]}
        ]
    })
    .to_string()
}

/// JSON table for the `status --json` view
pub fn status_table(
    address: &str,
    transport: &str,
    pin: &Pinconf,
    direction: Direction,
    level: bool,
    ignition: bool,
    bus_info: &str,
) -> String {
    let rows = vec![
        vec!["Address".to_string(), address.to_string()],
        vec!["Transport".to_string(), transport.to_string()],
        vec!["Port".to_string(), pin.port.to_string()],
        vec!["Pin".to_string(), pin.pin.to_string()],
        vec!["Direction (chip)".to_string(), direction.to_string()],
        vec!["Direction (config)".to_string(), pin.direction.to_string()],
        vec!["Logic".to_string(), pin.logic().to_string()],
        vec![
            "Level".to_string(),
            if level { "high" } else { "low" }.to_string(),
        ],
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
    use crate::tools::{Pin, Port};

    #[test]
    fn manifest_is_valid_for_the_tui_protocol() {
        let value: serde_json::Value = serde_json::from_str(&json(&Pinconf::default())).unwrap();
        assert_eq!(value["protocol"], 1);
        assert_eq!(value["views"].as_array().unwrap().len(), 3);
        assert_eq!(value["actions"].as_array().unwrap().len(), 3);

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
        assert_eq!(actions[2]["id"], "configure");
        assert_eq!(actions[2]["device"], expected);
    }

    #[test]
    fn configure_defaults_follow_the_saved_configuration() {
        let ignition = Pinconf {
            port: Port::Porta,
            pin: Pin::Pin4,
            direction: Direction::Input,
            inverted: true,
        };
        let value: serde_json::Value = serde_json::from_str(&json(&ignition)).unwrap();
        let args = value["actions"][2]["args"].as_array().unwrap();
        assert_eq!(args[0]["default"], "porta");
        assert_eq!(args[1]["default"], "pin4");
        assert_eq!(args[2]["default"], "input");
        assert_eq!(args[3]["default"], "true");
        assert_eq!(args[1]["options"].as_array().unwrap().len(), 8);

        // The action saves the new values: never pass --no-save
        let command = value["actions"][2]["command"].as_array().unwrap();
        assert!(!command.iter().any(|arg| arg == "--no-save"), "{command:?}");
        assert!(command.iter().any(|arg| arg == "--inverted"), "{command:?}");
    }

    #[test]
    fn status_and_metrics_parse() {
        let pin = Pinconf {
            port: Port::Portb,
            pin: Pin::Pin1,
            direction: Direction::Input,
            inverted: true,
        };
        let table: serde_json::Value = serde_json::from_str(&status_table(
            "0x20",
            "broker (shared with the TUI)",
            &pin,
            Direction::Output,
            false,
            true,
            "MCP rev A\nfirmware 1.0",
        ))
        .unwrap();
        assert_eq!(table["columns"][0], "Field");
        assert_eq!(table["rows"][2][1], "portb");
        assert_eq!(table["rows"][3][1], "pin1");
        assert_eq!(table["rows"][4][1], "output");
        assert_eq!(table["rows"][5][1], "input");
        assert_eq!(table["rows"][6][1], "inverted (active low)");
        assert_eq!(table["rows"][7][1], "low");
        assert_eq!(table["rows"][8][1], "ON");
        assert_eq!(table["rows"][9][1], "MCP rev A firmware 1.0");

        let on: serde_json::Value = serde_json::from_str(&metrics(true)).unwrap();
        assert_eq!(on["metrics"][0]["name"], "Ignition");
        assert_eq!(on["metrics"][0]["value"], 1.0);
        let off: serde_json::Value = serde_json::from_str(&metrics(false)).unwrap();
        assert_eq!(off["metrics"][0]["value"], 0.0);
    }
}
