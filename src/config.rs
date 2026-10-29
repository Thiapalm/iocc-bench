//! Configuration stored in the shared `config.yaml`.
//!
//! `iocc` exports `IOCC_CONFIG` with the path of its config file. This plugin
//! reads only its own top level section (`bench:`); every other section,
//! comment and indentation is left untouched.

use crate::tools::{Direction, Pin, Port};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::fs;
use std::path::PathBuf;

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
    /// Active-low logic: ON drives the pin low and OFF drives it high. The
    /// bench uses a normally closed relay, so the default is inverted.
    pub inverted: bool,
}

impl Default for Pinconf {
    fn default() -> Self {
        Pinconf {
            port: Port::Portb,
            pin: Pin::Pin1,
            direction: Direction::Output,
            inverted: true,
        }
    }
}

impl Pinconf {
    /// Physical level to write for a logical ignition state: with inverted
    /// logic (normally closed relay) ON is written as low and OFF as high.
    pub fn level(&self, on: bool) -> u8 {
        (on ^ self.inverted) as u8
    }

    /// Logical ignition state read from a physical level
    pub fn ignition(&self, level: bool) -> bool {
        level != self.inverted
    }

    /// Human readable logic mode, used by `status` and `set`
    pub fn logic(&self) -> &'static str {
        if self.inverted {
            "inverted (active low)"
        } else {
            "normal (active high)"
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

/// Same as [`read`], but silent when the file or section is missing. Used by
/// the manifest request, where a warning would only be noise.
pub fn read_quiet() -> BenchConf {
    read_with(false)
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

/// Persists one `bench.ignition.<key>` entry (`port`, `pin`, `direction`) in
/// the shared `config.yaml` and returns the file that was written.
///
/// Only that line is rewritten: comments, indentation and every other section
/// are kept byte for byte. A missing `bench`/`ignition` block or key is
/// created, so a change is never lost when the file was edited by hand.
pub fn save_ignition_entry(key: &str, value: &str) -> std::result::Result<PathBuf, String> {
    let Some(path) = std::env::var_os("IOCC_CONFIG") else {
        return Err("IOCC_CONFIG not set, cannot save the configuration".into());
    };
    let path = PathBuf::from(path);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let updated = replace_entry(&text, &[SECTION, "ignition"], key, value);
    fs::write(&path, updated)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(path)
}

/// Updates `key: value` inside the nested `bench.ignition` block, creating the
/// missing blocks when needed and keeping inline comments and indentation.
fn replace_entry(text: &str, path: &[&str], key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    upsert_entry(&mut lines, path, key, value);
    let mut content = lines.join(newline);
    if text.ends_with('\n') || text.is_empty() {
        content.push_str(newline);
    }
    content
}

/// Walks `path` and either rewrites the `key:` entry or inserts the missing
/// sections plus the entry at the end of the deepest existing block
fn upsert_entry(lines: &mut Vec<String>, path: &[&str], key: &str, value: &str) {
    let mut low = 0;
    let mut high = lines.len();
    let mut parent_indent = None;

    for (depth, segment) in path.iter().enumerate() {
        match find_block(lines, low, high, segment, parent_indent) {
            Some((header, indent, end)) => {
                low = header + 1;
                high = end;
                parent_indent = Some(indent);
            }
            None => {
                insert_block(lines, high, &path[depth..], parent_indent, key, value);
                return;
            }
        }
    }

    let block_indent = parent_indent.unwrap_or(0);
    let indentation = child_indent(lines, low, high, block_indent);
    let prefix = format!("{key}:");

    let target = (low..high).find(|&index| {
        let line = &lines[index];
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        let indent = line.len() - trimmed.len();
        indent == indentation && trimmed.starts_with(&prefix)
    });

    if let Some(index) = target {
        let trimmed = lines[index].trim_start();
        let indent = lines[index].len() - trimmed.len();
        let rest = trimmed.strip_prefix(&prefix).unwrap_or_default();
        // Keep the inline comment as it was (e.g. `# saved by the TUI`)
        let comment = rest
            .find('#')
            .map(|at| format!(" {}", &rest[at..]))
            .unwrap_or_default();
        lines[index] = format!("{}{key}: {value}{comment}", " ".repeat(indent));
        return;
    }

    let insert_at = (low..high)
        .rev()
        .find(|&index| !lines[index].trim().is_empty())
        .map(|index| index + 1)
        .unwrap_or(low);
    lines.insert(
        insert_at,
        format!("{}{key}: {value}", " ".repeat(indentation)),
    );
}

/// Finds `segment:` inside `lines[low..high]`. The first level must be at
/// column zero, the nested ones must be indented below the parent block.
/// Returns `(header index, header indent, end index)`.
fn find_block(
    lines: &[String],
    low: usize,
    high: usize,
    segment: &str,
    parent_indent: Option<usize>,
) -> Option<(usize, usize, usize)> {
    let prefix = format!("{segment}:");
    let header = (low..high).find(|&index| {
        let line = &lines[index];
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        let indent = line.len() - trimmed.len();
        let level = match parent_indent {
            Some(parent) => indent > parent,
            None => indent == 0,
        };
        level && trimmed.starts_with(&prefix)
    })?;

    let indent = lines[header].len() - lines[header].trim_start().len();
    let end = (header + 1..high)
        .find(|&index| {
            let line = &lines[index];
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return false;
            }
            line.len() - trimmed.len() <= indent
        })
        .unwrap_or(high);
    Some((header, indent, end))
}

/// Indentation of the direct children of a block, inferred from the file
fn child_indent(lines: &[String], low: usize, high: usize, block_indent: usize) -> usize {
    (low..high)
        .find_map(|index| {
            let trimmed = lines[index].trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let indent = lines[index].len() - trimmed.len();
            (indent > block_indent).then_some(indent)
        })
        .unwrap_or(block_indent + 2)
}

/// Creates the missing sections and the entry at `insert_at`, using two spaces
/// per level (or the parent indentation, on hand formatted files)
fn insert_block(
    lines: &mut Vec<String>,
    insert_at: usize,
    path: &[&str],
    parent_indent: Option<usize>,
    key: &str,
    value: &str,
) {
    let base = parent_indent.map_or(0, |indent| indent + 2);
    let mut block: Vec<String> = Vec::new();
    if insert_at == lines.len() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
        block.push(String::new());
    }
    for (offset, segment) in path.iter().enumerate() {
        block.push(format!("{}{segment}:", " ".repeat(base + 2 * offset)));
    }
    block.push(format!(
        "{}{key}: {value}",
        " ".repeat(base + 2 * path.len())
    ));
    for (offset, line) in block.into_iter().enumerate() {
        lines.insert(insert_at + offset, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "iocc:\n  verbose: true\nbench:\n  address: 0x21\n  verbose: true\n  ignition:\n    port: porta\n    pin: pin3\n    direction: input\n    inverted: true\n";

    #[test]
    fn reads_the_bench_section() {
        let root: RootConfig = serde_yaml::from_str(SAMPLE).unwrap();
        assert_eq!(root.bench.address, Address(0x21));
        assert!(root.bench.verbose);
        assert_eq!(root.bench.ignition.port, Port::Porta);
        assert_eq!(root.bench.ignition.pin, Pin::Pin3);
        assert_eq!(root.bench.ignition.direction, Direction::Input);
        assert!(root.bench.ignition.inverted);
    }

    #[test]
    fn missing_section_and_fields_use_the_defaults() {
        let root: RootConfig = serde_yaml::from_str("iocc:\n  verbose: true\n").unwrap();
        assert_eq!(root.bench.address, Address(0x20));
        assert_eq!(root.bench.ignition.port, Port::Portb);
        assert_eq!(root.bench.ignition.pin, Pin::Pin1);
        assert_eq!(root.bench.ignition.direction, Direction::Output);
        assert!(root.bench.ignition.inverted);

        let partial: RootConfig = serde_yaml::from_str("bench:\n  verbose: true\n").unwrap();
        assert_eq!(partial.bench.address, Address(0x20));
        assert!(partial.bench.verbose);
    }

    #[test]
    fn inverted_logic_maps_the_pin_levels() {
        let normal = Pinconf {
            inverted: false,
            ..Pinconf::default()
        };
        assert_eq!(normal.level(true), 1);
        assert_eq!(normal.level(false), 0);
        assert!(normal.ignition(true));
        assert!(!normal.ignition(false));
        assert_eq!(normal.logic(), "normal (active high)");

        // Normally closed relay: ON is low and OFF is high (bench default)
        let inverted = Pinconf::default();
        assert!(inverted.inverted);
        assert_eq!(inverted.level(true), 0);
        assert_eq!(inverted.level(false), 1);
        assert!(inverted.ignition(false));
        assert!(!inverted.ignition(true));
        assert_eq!(inverted.logic(), "inverted (active low)");
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

    const CONFIG: &str = "iocc:\n  verbose: true\n\nbench:\n  address: 0x20\n  ignition:\n    # keep me\n    port: portb\n    pin: pin1\n    direction: output # default\n\nother:\n  x: 1\n";

    #[test]
    fn replaces_only_the_requested_entry() {
        let text = replace_entry(CONFIG, &[SECTION, "ignition"], "port", "porta");
        assert!(text.contains("port: porta"), "{text}");
        assert!(text.contains("pin: pin1"), "{text}");
        assert!(text.contains("direction: output # default"), "{text}");
        assert!(text.contains("# keep me"), "{text}");
        assert!(text.contains("other:"), "{text}");
    }

    #[test]
    fn inserts_a_missing_key_inside_the_block() {
        let text = replace_entry(CONFIG, &[SECTION, "ignition"], "pullup", "true");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[9], "    direction: output # default");
        assert_eq!(lines[10], "    pullup: true");
        assert_eq!(lines[11], "");
        assert_eq!(lines[12], "other:");
    }

    #[test]
    fn creates_the_missing_sections() {
        let text = replace_entry(
            "iocc:\n  verbose: true\n",
            &[SECTION, "ignition"],
            "direction",
            "input",
        );
        assert!(
            text.contains("\nbench:\n  ignition:\n    direction: input\n"),
            "{text}"
        );
        let root: RootConfig = serde_yaml::from_str(&text).unwrap();
        assert_eq!(root.bench.ignition.direction, Direction::Input);
        assert!(text.contains("verbose: true"));
    }

    #[test]
    fn creates_the_block_when_only_the_parent_exists() {
        let text = replace_entry(
            "bench:\n  address: 0x20\n",
            &[SECTION, "ignition"],
            "pin",
            "pin2",
        );
        let root: RootConfig = serde_yaml::from_str(&text).unwrap();
        assert_eq!(root.bench.ignition.pin, Pin::Pin2);
        assert_eq!(root.bench.address, Address(0x20));
    }

    #[test]
    fn saves_the_shared_file() {
        let dir = std::env::temp_dir().join(format!("iocc-bench-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        fs::write(&path, CONFIG).unwrap();
        // SAFETY: this is the only test that touches IOCC_CONFIG
        unsafe { std::env::set_var("IOCC_CONFIG", &path) };
        let saved = save_ignition_entry("pin", "pin4").unwrap();
        save_ignition_entry("inverted", "true").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        unsafe { std::env::remove_var("IOCC_CONFIG") };
        assert_eq!(saved, path);
        assert!(text.contains("pin: pin4"), "{text}");
        assert!(text.contains("inverted: true"), "{text}");
        assert!(
            text.contains("# keep me") && text.contains("other:"),
            "{text}"
        );
        let root: RootConfig = serde_yaml::from_str(&text).unwrap();
        assert_eq!(root.bench.ignition.pin, Pin::Pin4);
        assert!(root.bench.ignition.inverted);
        let _ = fs::remove_dir_all(&dir);
    }
}
