//! Configuration stored in the shared `config.yaml`.
//!
//! `iocc` exports `IOCC_CONFIG` with the path of its config file. This plugin
//! reads only its own top level section (`bench:`); every other section,
//! comment and indentation is left untouched.

use crate::tools::{Direction, Pin, Port};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::fs;

/// Top level section read by this plugin. `iocc` itself ignores it.
pub const SECTION: &str = "bench";

/// 7 bit I2C address, written as `0x20` in `config.yaml`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(pub u8);

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// YAML may read `0x20` as an integer or keep it as a string. Accept
        /// both so the usual `config.yaml` spelling keeps working.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Text(String),
            Number(u8),
        }

        match Value::deserialize(deserializer)? {
            Value::Text(text) => parse_address(&text)
                .map(Address)
                .map_err(serde::de::Error::custom),
            Value::Number(value) => Ok(Address(value)),
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:02X}", self.0)
    }
}

/// Accepts hexadecimal text such as `0x20` or `20`
fn parse_address(text: &str) -> std::result::Result<u8, String> {
    let text = text.trim();
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    u8::from_str_radix(hex, 16).map_err(|error| format!("invalid I2C address '{text}': {error}"))
}

/// One MCP23017 output pin driven by the plugin
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Pinconf {
    pub port: Port,
    pub pin: Pin,
    pub direction: Direction,
}

impl Default for Pinconf {
    fn default() -> Self {
        Pinconf {
            port: Port::Portb,
            pin: Pin::Pin1,
            direction: Direction::Output,
        }
    }
}

/// The `bench:` section. Missing fields use these defaults, so a partial (or
/// absent) section keeps working.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BenchConf {
    pub address: Address,
    pub verbose: bool,
    pub i2c_speed_hz: u32,
    pub reset_on_open: bool,
    /// Timeout in milliseconds
    pub timeout: u64,
    /// Pin switched by `on`/`off`
    pub ignition: Pinconf,
}

impl Default for BenchConf {
    fn default() -> Self {
        BenchConf {
            address: Address(0x20),
            verbose: false,
            i2c_speed_hz: 400_000,
            reset_on_open: false,
            timeout: 10,
            ignition: Pinconf::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RootConfig {
    #[serde(default)]
    bench: BenchConf,
}

/// Reads the section, warning on unreadable or invalid files
pub fn read() -> BenchConf {
    read_with(true)
}

fn read_with(warn: bool) -> BenchConf {
    let Some(path) = std::env::var_os("IOCC_CONFIG") else {
        if warn {
            eprintln!("Warning: IOCC_CONFIG not set; using the default configuration");
        }
        return BenchConf::default();
    };

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => {
            if warn {
                eprintln!(
                    "Warning: cannot read {}: {error}; using the default configuration",
                    std::path::Path::new(&path).display()
                );
            }
            return BenchConf::default();
        }
    };

    match serde_yaml::from_str::<RootConfig>(&content) {
        Ok(config) => config.bench,
        Err(error) => {
            if warn {
                eprintln!("Warning: invalid {SECTION} configuration: {error}; using the defaults");
            }
            BenchConf::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "iocc:\n  verbose: true\nbench:\n  address: 0x21\n  verbose: true\n  ignition:\n    port: porta\n    pin: pin3\n    direction: input\n";

    #[test]
    fn reads_the_bench_section() {
        let root: RootConfig = serde_yaml::from_str(SAMPLE).unwrap();
        assert_eq!(root.bench.address, Address(0x21));
        assert!(root.bench.verbose);
        assert_eq!(root.bench.ignition.port, Port::Porta);
        assert_eq!(root.bench.ignition.pin, Pin::Pin3);
        assert_eq!(root.bench.ignition.direction, Direction::Input);
    }

    #[test]
    fn missing_section_and_fields_use_the_defaults() {
        let root: RootConfig = serde_yaml::from_str("iocc:\n  verbose: true\n").unwrap();
        assert_eq!(root.bench.address, Address(0x20));
        assert_eq!(root.bench.ignition.port, Port::Portb);
        assert_eq!(root.bench.ignition.pin, Pin::Pin1);
        assert_eq!(root.bench.ignition.direction, Direction::Output);

        let partial: RootConfig = serde_yaml::from_str("bench:\n  verbose: true\n").unwrap();
        assert_eq!(partial.bench.address, Address(0x20));
        assert!(partial.bench.verbose);
    }

    #[test]
    fn address_accepts_both_spellings() {
        assert_eq!(parse_address("0x20"), Ok(0x20));
        assert_eq!(parse_address("2a"), Ok(0x2A));
        assert!(parse_address("nope").is_err());
        assert_eq!(Address(0x3C).to_string(), "0x3C");

        // `0x20` stays a string and is read as hexadecimal; a plain integer is
        // taken as decimal, while a quoted `"20"` is still hexadecimal.
        let hex: BenchConf = serde_yaml::from_str("address: 0x20\n").unwrap();
        let decimal: BenchConf = serde_yaml::from_str("address: 32\n").unwrap();
        let quoted: BenchConf = serde_yaml::from_str("address: \"20\"\n").unwrap();
        assert_eq!(hex.address, Address(0x20));
        assert_eq!(decimal.address, Address(32));
        assert_eq!(quoted.address, Address(0x20));
    }
}
