//! `iocc-bench` plugin.
//!
//! Controls an ignition output wired to an MCP23017 expander on the MCP2221
//! bus. The executable is named `iocc-bench`, so `iocc bench <command>` runs it;
//! `iocc tui` also finds it and asks for the manifest without recompiling iocc.

use clap::{CommandFactory, Parser, Subcommand};
use std::io::Write;
use std::process::exit;
use std::time::Duration;

mod bus;
use bus::Bus;
mod config;
use config::{BenchConf, Pinconf, save_ignition_entry};
mod manifest;
mod tools;
use tools::{Direction, Pin, Port, pin_dir, read_pin, set_pin_dir, write_pin};

#[derive(Parser)]
#[command(version, about = "Control the bench ignition output through iocc", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Turn the ignition output ON
    On,
    /// Turn the ignition output OFF
    Off,
    /// Read the current ignition state
    Status {
        /// Print a JSON table (used by the iocc TUI)
        #[arg(long)]
        json: bool,
    },
    /// Keep printing the ignition state until interrupted (TUI live panel)
    Watch {
        /// Milliseconds between samples
        #[arg(long, default_value_t = 500)]
        interval_ms: u64,
        /// Print one JSON metrics object per line
        #[arg(long)]
        json: bool,
    },
    /// Change the ignition pin configuration and save it in config.yaml
    Set {
        /// Expander port: porta or portb
        #[arg(long)]
        port: Option<Port>,
        /// Expander pin: pin0 to pin7
        #[arg(long)]
        pin: Option<Pin>,
        /// Pin direction: input or output
        #[arg(long)]
        direction: Option<Direction>,
        /// Inverted logic: true drives the pin low for ON (normally closed relay)
        #[arg(long, value_name = "BOOL")]
        inverted: Option<bool>,
        /// Do not write the change to config.yaml (temporary change)
        #[arg(long)]
        no_save: bool,
    },
}

fn main() {
    // Answered before anything else: the TUI asks for it without opening the
    // device and without IOCC_HOST (the flag is handled here, not by iocc).
    if std::env::args().any(|arg| arg == manifest::MANIFEST_FLAG) {
        println!("{}", manifest::json(&config::read_quiet().ignition));
        return;
    }

    // Plugins are not standalone: iocc sets IOCC_HOST when it starts them.
    if std::env::var_os("IOCC_HOST").is_none() {
        eprintln!("iocc-bench is a plugin and must be run through iocc: iocc bench <command>");
        exit(1);
    }

    let cli = Cli::parse();
    let config = config::read();
    if config.verbose {
        println!("[bench] Configuration: {config:#?}");
    }
    if let Err(error) = run(&cli, &config) {
        eprintln!("Error: {error}");
        exit(1);
    }
}

fn run(cli: &Cli, config: &BenchConf) -> bus::Result<()> {
    let Some(command) = &cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    let mut bus = Bus::open(config.i2c_speed_hz, config.reset_on_open, config.timeout)?;
    match command {
        Commands::On => switch(&mut bus, config, true),
        Commands::Off => switch(&mut bus, config, false),
        Commands::Status { json } => status(&mut bus, config, *json),
        Commands::Watch { interval_ms, json } => watch(&mut bus, config, *interval_ms, *json),
        Commands::Set {
            port,
            pin,
            direction,
            inverted,
            no_save,
        } => set(
            &mut bus, config, *port, *pin, *direction, *inverted, *no_save,
        ),
    }
}

/// Drives the configured pin, applying the direction (and the inverted logic,
/// when enabled) from `config.yaml` first
fn switch(bus: &mut Bus, config: &BenchConf, on: bool) -> bus::Result<()> {
    let address = config.address.0;
    let pin = config.ignition;
    set_pin_dir(bus, address, pin.port, pin.pin, pin.direction)?;
    write_pin(bus, address, pin.port, pin.pin, pin.level(on))?;
    println!("Ignition {}", if on { "ON" } else { "OFF" });
    Ok(())
}

/// Applies the new pin configuration to the chip and persists it in the shared
/// `config.yaml` (only the changed `bench.ignition.*` lines), like the power
/// monitor does with the operating mode. `--no-save` keeps it in memory.
fn set(
    bus: &mut Bus,
    config: &BenchConf,
    port: Option<Port>,
    pin: Option<Pin>,
    direction: Option<Direction>,
    inverted: Option<bool>,
    no_save: bool,
) -> bus::Result<()> {
    if port.is_none() && pin.is_none() && direction.is_none() && inverted.is_none() {
        return Err("nothing to change: pass --port, --pin, --direction and/or --inverted".into());
    }

    let mut ignition = config.ignition;
    if let Some(port) = port {
        ignition.port = port;
    }
    if let Some(pin) = pin {
        ignition.pin = pin;
    }
    if let Some(direction) = direction {
        ignition.direction = direction;
    }
    if let Some(inverted) = inverted {
        ignition.inverted = inverted;
    }

    // The chip accepts the change first, so the file always matches the device
    set_pin_dir(
        bus,
        config.address.0,
        ignition.port,
        ignition.pin,
        ignition.direction,
    )?;
    print_ignition(&ignition);

    if no_save {
        println!("(temporary; without --no-save it is kept in config.yaml)");
        return Ok(());
    }

    let mut saved = None;
    for (changed, key, value) in [
        (port.is_some(), "port", ignition.port.to_string()),
        (pin.is_some(), "pin", ignition.pin.to_string()),
        (
            direction.is_some(),
            "direction",
            ignition.direction.to_string(),
        ),
        (
            inverted.is_some(),
            "inverted",
            ignition.inverted.to_string(),
        ),
    ] {
        if changed {
            saved = Some(save_ignition_entry(key, &value)?);
        }
    }
    if let Some(path) = saved {
        println!("Saved in {}", path.display());
    }
    Ok(())
}

fn print_ignition(ignition: &Pinconf) {
    println!(
        "Ignition pin: {} {} {}, {}",
        ignition.port,
        ignition.pin,
        ignition.direction,
        ignition.logic()
    );
}

fn status(bus: &mut Bus, config: &BenchConf, json: bool) -> bus::Result<()> {
    let address = config.address.0;
    let pin = config.ignition;
    let direction = pin_dir(bus, address, pin.port, pin.pin)?;
    let level = read_pin(bus, address, pin.port, pin.pin)?;
    let ignition = pin.ignition(level);
    let info = bus.status()?;

    if json {
        println!(
            "{}",
            manifest::status_table(
                &config.address.to_string(),
                bus.transport(),
                &pin,
                direction,
                level,
                ignition,
                &info,
            )
        );
        return Ok(());
    }

    println!("Address   : {}", config.address);
    println!("Transport : {}", bus.transport());
    println!("Port      : {}", pin.port);
    println!("Pin       : {}", pin.pin);
    println!("Direction : {direction} (chip), {} (config)", pin.direction);
    println!("Logic     : {}", pin.logic());
    println!("Level     : {}", if level { "high" } else { "low" });
    println!("Ignition  : {}", if ignition { "ON" } else { "OFF" });
    println!("Bus       : {}", info.replace('\n', " "));
    Ok(())
}

fn watch(bus: &mut Bus, config: &BenchConf, interval_ms: u64, json: bool) -> bus::Result<()> {
    let address = config.address.0;
    let pin = config.ignition;
    let interval = Duration::from_millis(interval_ms.max(10));
    loop {
        let ignition = pin.ignition(read_pin(bus, address, pin.port, pin.pin)?);
        if json {
            println!("{}", manifest::metrics(ignition));
        } else {
            println!("{}", if ignition { "ON" } else { "OFF" });
        }
        // Streaming views read one sample per stdout line, so flush every line.
        std::io::stdout().flush()?;
        std::thread::sleep(interval);
    }
}
