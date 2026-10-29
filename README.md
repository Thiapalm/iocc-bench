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
    # Normally closed relay on the bench: ON drives the pin low
    inverted: true
```

| Field | Notes |
|---|---|
| `address` | 7 bit I2C address of the expander (`0x20`) |
| `i2c_speed_hz` / `timeout` | MCP2221 I2C speed and timeout (`timeout` in milliseconds) |
| `reset_on_open` | resets the MCP2221 when the plugin opens it |
| `ignition.port` | `porta` or `portb` |
| `ignition.pin` | `pin0` to `pin7` |
| `ignition.direction` | `output` (normal) or `input` |
| `ignition.inverted` | `true` (default, ON = low for the normally closed relay) or `false` (ON = high) |

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

The installer also merges the `bench:` section of this repository's `config.yaml` into the shared
`config.yaml` (`iocc config import`), so the defaults are ready on the first run. Sections that
already exist are kept; use `iocc config import --force config.yaml` to replace one.

## Commands

| Command | Output |
|---|---|
| `iocc bench on` | turns the ignition ON, applying the direction and pin logic (NC relay: low) |
| `iocc bench off` | turns the ignition OFF, applying the direction and pin logic (NC relay: high) |
| `iocc bench set [--port P] [--pin P] [--direction D] [--inverted BOOL] [--no-save]` | changes the ignition pin configuration, applies it to the chip and saves it in `config.yaml` |
| `iocc bench status [--json]` | address, transport, port, pin, direction (chip and config), logic, pin level and logical state |
| `iocc bench watch [--json] [--interval-ms 500]` | keeps printing the logical state until interrupted |

`--json` prints the machine readable form used by the TUI (`status` as a table, `watch` as
metrics). `watch` prints one line per sample and keeps running until interrupted; `--interval-ms`
accepts values from 10 ms up (default 500).

`on` and `off` apply `ignition.direction` before writing, so an `input` pin only changes the
output latch and does not drive the line. With `ignition.inverted: true` (the normally closed
relay of the bench) the levels are swapped: `on` writes low and `off` writes high. `status` and
`watch` always report the **logical** state, and `status` shows the configured logic. `status` only
reads; it reports the direction configured in the chip (IODIR) next to the one in `config.yaml` and
does not reconfigure the pin.

## Saved settings

`set` applies the new port/pin/direction/logic to the chip and writes the changed values to the
`bench.ignition` section of `config.yaml` (only those lines: comments, indentation and the other
sections are kept). The next command and the TUI prompt start with the saved values. Use
`--no-save` for a temporary change:

```sh
iocc bench set --pin pin3                    # ignition pin: portb pin3 output, saved
iocc bench set --direction input             # same pin, now an input, saved
iocc bench set --inverted true               # normally closed relay: ON = low, saved
iocc bench set --port porta --no-save        # this run only
```

The plugin receives the config path from iocc through `IOCC_CONFIG`; outside `iocc` it cannot read
or save the shared file. A missing `bench`/`ignition` block is created instead of losing the
change.

## TUI

`iocc tui` lists bench with three panels and three actions:

| Panel | What it shows |
|---|---|
| **Ignition state** (`metrics`, streaming every 500 ms) | current level; pin it with `p` to keep it in *Watches* and record it with `g` |
| **Ignition graph** (`chart`, streaming every 500 ms) | the level over time with the y axis fixed at `0` and `1` |
| **Status** (`table`, every 2 s) | address, transport, port, pin, direction (chip and config), logic, level (high/low) and board info |

**Ignition ON** and **Ignition OFF** run without confirmation. Both are `shared`: they talk to the
TUI broker (`IOCC_SOCKET`) instead of opening the MCP2221, so the panels keep sampling while another
plugin uses the bus.

**Configure ignition pin** prompts for the port, pin, direction and inverted logic, using the
values saved in `config.yaml` as defaults. It applies the direction to the chip and keeps the new
values in `bench.ignition`, so the next prompt shows them.

The plugin describes itself through `--iocc-manifest`; see
[`docs/tui-plugins.md`](https://github.com/Thiapalm/iocc/blob/tui/docs/tui-plugins.md) for the protocol. Pinning, zooming and CSV
recording work as in the other plugins: `[`/`]` selects a panel, `p` pins it, `x` unpins it and
`g` records the pinned `metrics` panels. The TUI panels require iocc 1.1.0 or newer; the fixed
`0..1` graph axis uses the `views[].min`/`max` fields added to iocc after 1.2.0. Older versions
still draw the graph, but with a padded axis.

### Command line and the broker

When a TUI is running, `iocc bench ...` receives `IOCC_SOCKET` automatically and drives the pin
through its broker, so the command line shares the MCP2221 with the panels:

```sh
iocc bench status --json
iocc bench watch --json
iocc bench on
```

Use `--direct` to open the device itself (the plugin receives no `IOCC_SOCKET`, even if the shell
exports one) or `--broker`/`--via-tui` to require a running broker. The socket is
`$XDG_RUNTIME_DIR/iocc.sock` unless the TUI was started with another `IOCC_SOCKET`.

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
| `USB error: Resource busy` | another process holds the MCP2221 (a running TUI, or another `iocc bench`). Close it, or use the running TUI: iocc finds its broker automatically. |
| `cannot connect to ... broker at ...` | `IOCC_SOCKET` is set but stale. Unset it to open the device directly. |
| `device in exclusive use by a plugin` | an exclusive view/action is using the chip in the TUI; wait for it to finish. |
| The TUI panel stops with `command exited` | `watch` ended (the device was unplugged or a communication error occurred). Press `R` in the TUI to restart it. |
| `iocc-bench is a plugin and must be run through iocc` | it was executed directly. Use `iocc bench ...`. |

## Contact

palmieri@adastra.aleeas.com
