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
use config::BenchConf;
mod manifest;
mod tools;
use tools::{pin_dir, read_pin, set_pin_dir, write_pin};

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
}

fn main() {
    // Answered before anything else: the TUI asks for it without opening the
    // device and without IOCC_HOST (the flag is handled here, not by iocc).
    if std::env::args().any(|arg| arg == manifest::MANIFEST_FLAG) {
        println!("{}", manifest::json());
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
    }
}

/// Drives the configured pin, applying the direction from `config.yaml` first
fn switch(bus: &mut Bus, config: &BenchConf, on: bool) -> bus::Result<()> {
    let address = config.address.0;
    let pin = config.ignition;
    set_pin_dir(bus, address, pin.port, pin.pin, pin.direction)?;
    write_pin(bus, address, pin.port, pin.pin, on as u8)?;
    println!("Ignition {}", if on { "ON" } else { "OFF" });
    Ok(())
}

fn status(bus: &mut Bus, config: &BenchConf, json: bool) -> bus::Result<()> {
    let address = config.address.0;
    let pin = config.ignition;
    let direction = pin_dir(bus, address, pin.port, pin.pin)?;
    let ignition = read_pin(bus, address, pin.port, pin.pin)?;
    let info = bus.status()?;

    if json {
        println!(
            "{}",
            manifest::status_table(
                &config.address.to_string(),
                bus.transport(),
                pin.port,
                pin.pin,
                direction,
                pin.direction,
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
    println!("Ignition  : {}", if ignition { "ON" } else { "OFF" });
    println!("Bus       : {}", info.replace('\n', " "));
    Ok(())
}

fn watch(bus: &mut Bus, config: &BenchConf, interval_ms: u64, json: bool) -> bus::Result<()> {
    let address = config.address.0;
    let pin = config.ignition;
    let interval = Duration::from_millis(interval_ms.max(10));
    loop {
        let ignition = read_pin(bus, address, pin.port, pin.pin)?;
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
