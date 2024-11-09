// Copyright © 2023-2024 Vouch.io LLC

use anyhow::{Error, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info, LevelFilter};
use serialport::{available_ports, SerialPortType};
use simplelog::{ColorChoice, Config, SimpleLogger, TermLogger, TerminalMode};
use std::env;
use std::path::PathBuf;
use std::process;

use mcumgr_client::*;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Device name
    #[arg(short, long, default_value = "")]
    device: String,

    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// Initial timeout in seconds
    #[arg(short = 't', long = "initial_timeout", default_value_t = 60)]
    initial_timeout_s: u32,

    /// Subsequent timeout in msec
    #[arg(short = 'u', long = "subsequent_timeout", default_value_t = 200)]
    subsequent_timeout_ms: u32,

    // Number of retry per packet
    #[arg(long, default_value_t = 4)]
    nb_retry: u32,

    /// Maximum length per line
    #[arg(short, long, default_value_t = 128)]
    linelength: usize,

    /// Maximum length per request
    #[arg(short, long, default_value_t = 512)]
    mtu: usize,

    /// Baudrate
    #[arg(short, long, default_value_t = 115_200)]
    baudrate: u32,

    #[command(subcommand)]
    command: Commands,
}

impl From<&Cli> for SerialSpecs {
    fn from(cli: &Cli) -> SerialSpecs {
        SerialSpecs {
            device: cli.device.clone(),
            initial_timeout_s: cli.initial_timeout_s,
            subsequent_timeout_ms: cli.subsequent_timeout_ms,
            nb_retry: cli.nb_retry,
            linelength: cli.linelength,
            mtu: cli.mtu,
            baudrate: cli.baudrate,
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// List slots on the device
    List,

    /// Reset the device
    Reset,

    /// Upload a file to the device
    Upload {
        filename: PathBuf,

        /// Slot number
        #[arg(short, long, default_value_t = 0)]
        slot: u8,
    },

    /// Test image againt given hash
    Test {
        hash: String,
        #[arg(short, long)]
        confirm: Option<bool>,
    },

    /// Erase image at slot
    Erase {
        #[arg(short, long)]
        slot: Option<u32>,
    },
}

/*
fn mcumgr_command(command: &Commands) -> Result<(), anyhow::Error> {
    let mut specs = SerialSpecs {
        device: "".to_string(),
        initial_timeout_s: 60,
        subsequent_timeout_ms: 200,
        nb_retry: 4,
        linelength: 128,
        mtu: 512,
        baudrate: 115_200,
    };

    let vid: u16 = 12259;
    let mcuboot_pid: u16 = 256;
    let application_pid: u16 = 10;

    match available_ports() {
        Ok(ports) => {
            for port in ports {
                match port.port_type {
                    SerialPortType::UsbPort(info) if info.vid == vid => {
                        if info.pid == mcuboot_pid {
                            info!(
                                "Found MCUBOOT device with serial {}",
                                info.serial_number.unwrap_or("n/a".to_string())
                            );
                            let name = port.port_name;
                            // on Mac, use only cu device
                            if env::consts::OS == "macos" {
                                if name.contains("cu.usbmodem") {
                                    specs.device = name;
                                    break;
                                }
                            } else {
                                specs.device = name;
                                break;
                            }
                        } else if info.pid == application_pid {
                            error!(
                                    "Found device with serial {} but bootloader was not enabled. Please hold button before inserting.",
                                    info.serial_number.unwrap_or("n/a".to_string())
                                );
                        }
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            error!("Error listing serial ports: {}", e);
            return Err(e.into());
        }
    }

    if specs.device.is_empty() {
        anyhow::bail!("No MCUBOOT device found.");
    }

    // execute command
    match command {
        Commands::List => || -> Result<(), Error> {
            let v = list(&specs)?;
            print!("response: {}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }(),
        Commands::Reset => reset(&specs),
        Commands::Upload { filename, slot } => || -> Result<(), Error> {
            // create a progress bar
            let pb = ProgressBar::new(1 as u64);
            pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap().progress_chars("=> "));

            upload(
                &specs,
                filename,
                *slot,
                Some(|offset, total| {
                    if let Some(l) = pb.length() {
                        if l != total {
                            pb.set_length(total as u64)
                        }
                    }

                    pb.set_position(offset as u64);

                    if offset >= total {
                        pb.finish_with_message("upload complete");
                    }
                }),
            )
        }(),
        Commands::Test { hash, confirm } => {
            || -> Result<(), Error> { test(&specs, hex::decode(hash)?, *confirm) }()
        }
        Commands::Erase { slot } => erase(&specs, *slot),
    }
}
*/

fn main() {
    // parse command line arguments
    let mut cli = Cli::parse();

    // initialize the logger with the desired level filter based on the verbose flag
    let level_filter = if cli.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    TermLogger::init(
        level_filter,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .unwrap_or_else(|_| SimpleLogger::init(LevelFilter::Info, Default::default()).unwrap());

    // if no device is specified, try to auto detect it
    if cli.device.is_empty() {
        let vid: u16 = 12259;
        let mcuboot_pid: u16 = 256;
        let application_pid: u16 = 10;
        match available_ports() {
            Ok(ports) => {
                for port in ports {
                    //info!("Found PORT {:?}", port);
                    match port.port_type {
                        SerialPortType::UsbPort(info) if info.vid == vid => {
                            if info.pid == mcuboot_pid {
                                info!(
                                    "Found MCUBOOT device with serial {}",
                                    info.serial_number.unwrap_or("n/a".to_string())
                                );
                                let name = port.port_name;
                                // on Mac, use only cu device
                                if env::consts::OS == "macos" {
                                    if name.contains("cu.usbmodem") {
                                        cli.device = name;
                                        break;
                                    }
                                } else {
                                    cli.device = name;
                                    break;
                                }
                            } else if info.pid == application_pid {
                                error!(
                                    "Found device with serial {} but bootloader was not enabled. Please hold button before inserting.",
                                    info.serial_number.unwrap_or("n/a".to_string())
                                );
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("Error listing serial ports: {}", e);
                process::exit(1);
            }
        }

        if cli.device.is_empty() {
            error!("No MCUBOOT device found.");
            process::exit(1);
        }
    }

    let specs = SerialSpecs::from(&cli);

    // execute command
    let result = match &cli.command {
        Commands::List => || -> Result<(), Error> {
            let v = list(&specs)?;
            print!("response: {}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }(),
        Commands::Reset => reset(&specs),
        Commands::Upload { filename, slot } => || -> Result<(), Error> {
            // create a progress bar
            let pb = ProgressBar::new(1 as u64);
            pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap().progress_chars("=> "));

            upload(
                &specs,
                filename,
                *slot,
                Some(|offset, total| {
                    if let Some(l) = pb.length() {
                        if l != total {
                            pb.set_length(total as u64)
                        }
                    }

                    pb.set_position(offset as u64);

                    if offset >= total {
                        pb.finish_with_message("upload complete");
                    }
                }),
            )
        }(),
        Commands::Test { hash, confirm } => {
            || -> Result<(), Error> { test(&specs, hex::decode(hash)?, *confirm) }()
        }
        Commands::Erase { slot } => erase(&specs, *slot),
    };

    // show error, if failed
    if let Err(e) = result {
        error!("Error: {}", e);
        process::exit(1);
    }
}
