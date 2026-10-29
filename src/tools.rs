use crate::bus::{Bus, Result};
use serde::Deserialize;
use std::fmt::{self, Display};

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Register {
    Iodir = 0x00,
    Ipol = 0x02,
    Gpinten = 0x04,
    Defval = 0x06,
    Intcon = 0x08,
    Iocon = 0x0A,
    Gppu = 0x0C,
    Intf = 0x0E,
    Intcap = 0x10,
    Gpio = 0x12,
    Olat = 0x14,
}

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Port {
    Porta = 0x00,
    Portb = 0x01,
}

impl Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Port::Porta => f.write_str("porta"),
            Port::Portb => f.write_str("portb"),
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pin {
    Pin0 = 0x01,
    Pin1 = 0x02,
    Pin2 = 0x04,
    Pin3 = 0x08,
    Pin4 = 0x10,
    Pin5 = 0x20,
    Pin6 = 0x40,
    Pin7 = 0x80,
    Invalid = 0x00,
}

impl Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Pin::Pin0 => "pin0",
            Pin::Pin1 => "pin1",
            Pin::Pin2 => "pin2",
            Pin::Pin3 => "pin3",
            Pin::Pin4 => "pin4",
            Pin::Pin5 => "pin5",
            Pin::Pin6 => "pin6",
            Pin::Pin7 => "pin7",
            Pin::Invalid => "invalid",
        };
        f.write_str(name)
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Output = 0x00,
    Input = 0xFF,
}

impl Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Output => f.write_str("output"),
            Direction::Input => f.write_str("input"),
        }
    }
}

impl std::str::FromStr for Port {
    type Err = String;

    fn from_str(text: &str) -> std::result::Result<Self, Self::Err> {
        match text {
            "porta" => Ok(Port::Porta),
            "portb" => Ok(Port::Portb),
            _ => Err(format!("invalid port '{text}': use porta or portb")),
        }
    }
}

impl std::str::FromStr for Pin {
    type Err = String;

    fn from_str(text: &str) -> std::result::Result<Self, Self::Err> {
        match text {
            "pin0" => Ok(Pin::Pin0),
            "pin1" => Ok(Pin::Pin1),
            "pin2" => Ok(Pin::Pin2),
            "pin3" => Ok(Pin::Pin3),
            "pin4" => Ok(Pin::Pin4),
            "pin5" => Ok(Pin::Pin5),
            "pin6" => Ok(Pin::Pin6),
            "pin7" => Ok(Pin::Pin7),
            _ => Err(format!("invalid pin '{text}': use pin0 to pin7")),
        }
    }
}

impl std::str::FromStr for Direction {
    type Err = String;

    fn from_str(text: &str) -> std::result::Result<Self, Self::Err> {
        match text {
            "input" => Ok(Direction::Input),
            "output" => Ok(Direction::Output),
            _ => Err(format!("invalid direction '{text}': use input or output")),
        }
    }
}

/**
 * Reads one byte from a register of one port of the MCP23017
 */
pub fn read_register(bus: &mut Bus, address: u8, reg: Register, port: Port) -> Result<u8> {
    let register = reg as u8 | port as u8;
    let data = bus.read(address, register, 1)?;
    data.first()
        .copied()
        .ok_or_else(|| format!("register 0x{register:02X} returned no data").into())
}

/**
 * Writes one byte to a register of one port of the MCP23017
 */
pub fn write_register(
    bus: &mut Bus,
    address: u8,
    port: Port,
    reg: Register,
    value: u8,
) -> Result<()> {
    bus.write(address, reg as u8 | port as u8, &[value])
}

/**
 * Configures the pin direction (IODIR) keeping the other pins as they are
 */
pub fn set_pin_dir(
    bus: &mut Bus,
    address: u8,
    port: Port,
    pin: Pin,
    direction: Direction,
) -> Result<()> {
    let mut result = read_register(bus, address, Register::Iodir, port)?;
    result = match direction {
        Direction::Input => bit_set(result, pin),
        Direction::Output => bit_clear(result, pin),
    };
    write_register(bus, address, port, Register::Iodir, result)
}

/**
 * Drives the output latch (OLAT) of the pin, keeping the other pins as they are
 */
pub fn write_pin(bus: &mut Bus, address: u8, port: Port, pin: Pin, value: u8) -> Result<()> {
    let mut result = read_register(bus, address, Register::Olat, port)?;
    result = match value {
        0 => bit_clear(result, pin),
        _ => bit_set(result, pin),
    };
    write_register(bus, address, port, Register::Olat, result)
}

/**
 * Reads the configured direction (IODIR) of the pin
 */
pub fn pin_dir(bus: &mut Bus, address: u8, port: Port, pin: Pin) -> Result<Direction> {
    let result = read_register(bus, address, Register::Iodir, port)?;
    Ok(if bit_read(result, pin) != 0 {
        Direction::Input
    } else {
        Direction::Output
    })
}

/**
 * Reads the current level of the pin from GPIO
 */
pub fn read_pin(bus: &mut Bus, address: u8, port: Port, pin: Pin) -> Result<bool> {
    let result = read_register(bus, address, Register::Gpio, port)?;
    Ok(bit_read(result, pin) != 0)
}

/**
 * This function is used to set a given bit. It must receive the byte to be changed
 * and the pin number to set
 */
pub fn bit_set(byte: u8, pin: Pin) -> u8 {
    byte | pin as u8
}

/**
 * This function is used to clear a given bit. It must receive the byte to be changed
 * and the pin number to be cleared
 */
pub fn bit_clear(byte: u8, pin: Pin) -> u8 {
    byte & !(pin as u8)
}

/**
 * This function reads a given bit from a byte. It must receive the byte and
 * the pin number to be read
 */
pub fn bit_read(byte: u8, pin: Pin) -> u8 {
    byte & pin as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_are_set_cleared_and_read_without_touching_the_others() {
        assert_eq!(bit_set(0b0000_0000, Pin::Pin3), 0b0000_1000);
        assert_eq!(bit_set(0b1111_1111, Pin::Pin3), 0b1111_1111);
        assert_eq!(bit_clear(0b1111_1111, Pin::Pin3), 0b1111_0111);
        assert_eq!(bit_clear(0b0000_0000, Pin::Pin3), 0b0000_0000);
        assert_ne!(bit_read(0b0000_1000, Pin::Pin3), 0);
        assert_eq!(bit_read(0b0000_0000, Pin::Pin3), 0);
    }

    #[test]
    fn config_names_are_printed_as_written_in_the_file() {
        assert_eq!(Port::Portb.to_string(), "portb");
        assert_eq!(Pin::Pin1.to_string(), "pin1");
        assert_eq!(Direction::Input.to_string(), "input");
    }
}
