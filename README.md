# iocc-bench

`iocc` plugin for a test bench **ignition output** wired to an MCP23017-compatible I/O expander
on the MCP2221 bus. It is not standalone: it only runs through `iocc bench ...`, which sets
`IOCC_HOST` and `IOCC_CONFIG`.

Settings live in the `bench:` section of the shared `config.yaml` (see `config.yaml`):

```yaml
bench:
  address: 0x20
  verbose: false
  i2c_speed_hz: 400000
  reset_on_open: false
  timeout: 10
  ignition:
    port: portb
    pin: pin1
    direction: output
```

| Field | Notes |
|---|---|
| `address` | 7 bit I2C address of the expander (`0x20`) |
| `i2c_speed_hz` / `timeout` | MCP2221 I2C speed and timeout (`timeout` in milliseconds) |
| `reset_on_open` | resets the MCP2221 when the plugin opens it |
| `ignition.port` | `porta` or `portb` |
| `ignition.pin` | `pin0` to `pin7` |
| `ignition.direction` | `output` (normal) or `input` |

Missing keys or a missing section fall back to these defaults.

## Install

Install `iocc` first. The script puts the plugin next to the `iocc` found in `PATH` (or in
`~/.local/bin` when iocc is not installed yet); use `--prefix DIR` to match a custom folder.

```sh
./install.sh                 # builds and installs next to iocc (or ~/.local/bin)
./install.sh --prefix DIR    # explicit folder
./install.sh --uninstall
```

The binary must be named `iocc-bench` and live in the same folder as `iocc` or in `PATH`.

## Commands

| Command | Output |
|---|---|
| `iocc bench on` | configures the pin direction and drives the output high |
| `iocc bench off` | configures the pin direction and drives the output low |
| `iocc bench status [--json]` | address, transport, port, pin, direction (chip and config) and current level |
| `iocc bench watch [--json] [--interval-ms 500]` | keeps printing the level until interrupted |

`--json` prints the machine readable form used by the TUI (`status` as a table, `watch` as
metrics). `watch` prints one line per sample and keeps running until interrupted; `--interval-ms`
accepts values from 10 ms up (default 500).

`on` and `off` apply `ignition.direction` before writing, so an `input` pin only changes the
output latch and does not drive the line. `status` only reads; it reports the direction configured
in the chip (IODIR) next to the one in `config.yaml` and does not reconfigure the pin.

## TUI

`iocc tui` lists bench with three panels and two actions:

| Panel | What it shows |
|---|---|
| **Ignition state** (`metrics`, streaming every 500 ms) | current level; pin it with `p` to keep it in *Watches* and record it with `g` |
| **Ignition graph** (`chart`, streaming every 500 ms) | the level over time with the y axis fixed at `0` and `1` |
| **Status** (`table`, every 2 s) | address, transport, port, pin, direction (chip and config), level and board info |

**Ignition ON** and **Ignition OFF** run without confirmation. Both are `shared`: they talk to the
TUI broker (`IOCC_SOCKET`) instead of opening the MCP2221, so the panels keep sampling while another
plugin uses the bus.

The plugin describes itself through `--iocc-manifest`; see
[`docs/tui-plugins.md`](https://github.com/Thiapalm/iocc/blob/tui/docs/tui-plugins.md) for the protocol. Pinning, zooming and CSV
recording work as in the other plugins: `[`/`]` selects a panel, `p` pins it, `x` unpins it and
`g` records the pinned `metrics` panels. The TUI panels require iocc 1.1.0 or newer; the fixed
`0..1` graph axis uses the `views[].min`/`max` fields added to iocc after 1.2.0. Older versions
still draw the graph, but with a padded axis.

### Command line and the broker

Command line use is unchanged: iocc does not set `IOCC_SOCKET` for `iocc bench ...`, so the plugin
opens the MCP2221 directly. To use a running TUI instead, export the socket yourself:

```sh
IOCC_SOCKET=/run/user/1000/iocc.sock iocc bench status --json
IOCC_SOCKET=/run/user/1000/iocc.sock iocc bench watch --json
```

The TUI socket is `$XDG_RUNTIME_DIR/iocc.sock` unless it was started with another `IOCC_SOCKET`.
This only works for plugins that implement the protocol (bench does).

### Dependency

The plugin uses [`iocc-bus`](https://github.com/Thiapalm/iocc/tree/tui/iocc-bus) for the broker
client. The crate is not published on crates.io yet, so `Cargo.toml` points at the `tui` branch:

```toml
iocc-bus = { git = "https://github.com/Thiapalm/iocc", branch = "tui" }
```

After the release, replace `branch = "tui"` with the matching tag. For local development next to a
checkout of iocc, a `path` dependency also works:

```toml
iocc-bus = { version = "0.1.0", path = "../iocc/iocc-bus" }
```

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `USB error: Resource busy` | another process holds the MCP2221 (a running TUI, or another `iocc bench`). Close it, or point `IOCC_SOCKET` at the running TUI. |
| `cannot connect to ... broker at ...` | `IOCC_SOCKET` is set but stale. Unset it to open the device directly. |
| `device in exclusive use by a plugin` | an exclusive view/action is using the chip in the TUI; wait for it to finish. |
| The TUI panel stops with `command exited` | `watch` ended (the device was unplugged or a communication error occurred). Press `R` in the TUI to restart it. |
| `iocc-bench is a plugin and must be run through iocc` | it was executed directly. Use `iocc bench ...`. |

## Contact

palmieri@adastra.aleeas.com
