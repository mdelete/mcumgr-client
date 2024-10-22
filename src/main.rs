// Copyright © 2023-2024 Vouch.io LLC

use anyhow::{Error, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info, LevelFilter};
use serialport::{available_ports, SerialPortType};
use simplelog::{ColorChoice, Config, SimpleLogger, TermLogger, TerminalMode};
use std::env;
//#[cfg(target_os = "macos")]
//use std::ops::Shr;
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
enum Commands {
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
        //let mut bootloaders = Vec::new();

        // match nusb::list_devices() {
        //     Ok(devices) => {
        //         for device in devices {
        //             if device
        //                 .product_string()
        //                 .unwrap_or_default()
        //                 .contains(&"MCUBOOT")
        //             {
        //                 #[cfg(target_os = "macos")]
        //                 info!(
        //                     "Found MCUBOOT device /dev/cu.usbmodem{:x}01", // this '01' append of the cdc driver is a long standing bug in osx
        //                     device.location_id().shr(16)
        //                 );
        //
        //                 #[cfg(windows)]
        //                 info!("Found WinUSB MCUBOOT device {:?}", device); // FIXME: how to get from WinUSB device name to COMx ???
        //             }
        //         }
        //     }
        //     Err(_) => {}
        // }

        match available_ports() {
            Ok(ports) => {
                for port in ports {
                    //info!("Found PORT {:?}", port);
                    match port.port_type {
                        SerialPortType::UsbPort(info) => {
                            if info.pid == 256 // MCUBOOT - FIXME: config value for this
                                && info.vid == 12259 // FIXME: config value for this
                                && info.product == Some("MCUBOOT".to_string())
                            {
                                info!(
                                    "Found MCUBOOT device with serial {}",
                                    info.serial_number.unwrap_or("".to_string())
                                );
                                let name = port.port_name;
                                // on Mac, use only special names
                                if env::consts::OS == "macos" {
                                    if name.contains("cu.usbmodem") {
                                        //bootloaders.push(name);
                                        cli.device = name;
                                        break;
                                    }
                                } else {
                                    //bootloaders.push(name);
                                    cli.device = name;
                                    break;
                                }
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

        // if there is one bootloader device, then use it
        //     if bootloaders.len() == 1 {
        //         cli.device = bootloaders[0].clone();
        //         info!(
        //             "One bootloader device found, setting device to: {}",
        //             cli.device
        //         );
        //     } else {
        //         // otherwise print all devices, and use a device, if there is only one device
        //         if cli.device.is_empty() {
        //             match available_ports() {
        //                 Ok(ports) => match ports.len() {
        //                     0 => {
        //                         error!("No serial port found.");
        //                         process::exit(1);
        //                     }
        //                     1 => {
        //                         cli.device = ports[0].port_name.clone();
        //                         info!(
        //                             "Only one serial port found, setting device to: {}",
        //                             cli.device
        //                         );
        //                     }
        //                     _ => {
        //                         error!("More than one serial port found, please specify one:");
        //                         for p in ports {
        //                             println!("{}", p.port_name);
        //                         }
        //                         process::exit(1);
        //                     }
        //                 },
        //                 Err(e) => {
        //                     println!("Error listing serial ports: {}", e);
        //                     process::exit(1);
        //                 }
        //             }
        //         }
        //     }
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
