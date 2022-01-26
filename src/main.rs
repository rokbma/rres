// Copyright (c) 2021 rokbma & the johncena141 hacker group on 1337x.to
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-only

use std::env;
use std::fs;
use std::path;
use std::process;

use drm::control::{Device as ControlDevice, Mode};
use drm::Device;
use eyre::WrapErr;
use simple_logger::SimpleLogger;

const USAGE: &str = "\
Usage: rres [options]

  -c, --card <card>\tSpecify a GPU (file existing in /dev/dri/, eg. card0)
  -m, --multi\t\tRead all monitors. If this option is ommited, rres will
             \t\treturn the resolution of the first detected monitor
  -v, --verbose\t\tVerbosity level. Can be specified multiple times, e.g. -vv
  -q, --quiet\t\tLower verbosity level. Opposite to -v
  -h, --help\t\tShow this help message

Environment variables:

  RRES_DISPLAY=<index>\tSelect display in single mode (starting at 0)

Wine Virtual Desktop example:

  wine \"explorer /desktop=Game,$(./rres)\" game.exe

";

// Card handle
// Really just to get a raw file descriptor for `drm`
pub struct Card(std::fs::File);

impl std::os::unix::io::AsRawFd for Card {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.0.as_raw_fd()
    }
}

impl Card {
    pub fn open<P: AsRef<path::Path>>(path: P) -> Self {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        options.write(true);
        Card(options.open(path).unwrap())
    }
}

// Implement `drm` types
impl Device for Card {}
impl ControlDevice for Card {}

fn main() -> eyre::Result<()> {
    // Settings
    let mut verbosity = log::LevelFilter::Warn;
    let mut multi = false;
    let mut card: Option<String> = None;

    // Handle CLI
    {
        use lexopt::prelude::*;
        let mut parser = lexopt::Parser::from_env();

        while let Some(arg) = parser.next()? {
            match arg {
                Short('m') | Long("multi") => {
                    multi = true;
                }
                Short('c') | Long("card") => {
                    card = Some(parser.value()?.into_string().unwrap());
                }
                Short('h') | Long("help") => {
                    println!("{}", USAGE);
                    process::exit(0);
                }
                Short('v') | Long("verbose") => {
                    verbosity = increment_loglevel(verbosity);
                }
                Short('q') | Long("quiet") => {
                    verbosity = decrement_loglevel(verbosity);
                }
                _ => return Err(eyre::eyre!("{}", arg.unexpected())),
            }
        }
    }

    // Init logger
    SimpleLogger::new().with_level(verbosity).init()?;

    // Store found displays
    let mut displays: Vec<Mode> = vec![];
    // Store the checked cards
    let mut cards: Vec<path::PathBuf> = vec![];

    if let Some(c) = card {
        // Open single card
        let mut file = path::PathBuf::from("/dev/dri/");
        file.push(&c);
        if !file.exists() || !c.starts_with("card") {
            return Err(eyre::eyre!("invalid card ({})", c));
        }
        cards.push(file);
    } else {
        // Open every card on the system
        for entry in fs::read_dir("/dev/dri/")? {
            let file = entry?;

            if let Some(name) = file.file_name().to_str() {
                if name.starts_with("card") {
                    cards.push(file.path());
                }
            }
        }
    }

    // Sort cards (card0, card1, card2...)
    cards.sort();

    // Read card list
    for file in cards {
        let gpu = Card::open(file);
        let info = gpu.get_driver()?;
        log::info!("Found GPU: {}", info.name().to_string_lossy());
        // Find displays
        match get_card_modes(gpu) {
            Ok(modes) => displays.extend_from_slice(&modes),
            Err(e) => log::error!("failed to read modes: {}", e),
        }
    }

    if displays.is_empty() {
        log::error!("found no display connected!");
        process::exit(1);
    }

    if multi {
        // List every display
        for (i, mode) in displays.iter().enumerate() {
            let res = mode.size();
            println!("Display #{}: {}x{}", i, res.0, res.1);
        }
    } else {
        let selection: usize = env::var("RRES_DISPLAY")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .wrap_err("Failed to parse RRES_DISPLAY")?;
        if selection > displays.len() - 1 {
            return Err(eyre::eyre!("invalid display: {}", selection));
        }
        // Print res of first display
        let res = displays[selection].size();
        println!("{}x{}", res.0, res.1);
    }

    Ok(())
}

/// Get all the connected display's modes from a libdrm card.
pub fn get_card_modes<G: ControlDevice>(gpu: G) -> eyre::Result<Vec<Mode>> {
    let mut modes: Vec<Mode> = vec![];

    let resources = gpu.resource_handles().wrap_err("failed to get resource handles")?;
    let connectors = resources.connectors();
    for handle in connectors {
        let connector = gpu.get_connector(*handle).wrap_err("failed to get connector handle")?;
        if connector.state() == drm::control::connector::State::Connected {
            // Connected, get mode
            modes.push(get_connector_mode(&gpu, connector)?);
        }
    }
    Ok(modes)
}

/// Get current display mode from connector
///
/// Note: nVidia GPUs don't share the current encoder+crtc, so this function will report the
/// native display's resolution instead of the current resolution.
fn get_connector_mode<G: ControlDevice>(
    gpu: &G,
    connector: drm::control::connector::Info,
) -> eyre::Result<Mode> {
    if connector.state() != drm::control::connector::State::Connected {
        return Err(eyre::eyre!("Connector is disconnected"));
    }
    if let Some(encoder_handle) = connector.current_encoder() {
        // Get the encoder then crtc
        let encoder = gpu.get_encoder(encoder_handle)?;
        if let Some(crtc_handle) = encoder.crtc() {
            let crtc = gpu.get_crtc(crtc_handle).wrap_err("failed to get crtc")?;
            // Get current mode, and store it
            if let Some(current_mode) = crtc.mode() {
                log::info!(
                    "Found display: {:?}, {}x{}",
                    connector.interface(),
                    current_mode.size().0,
                    current_mode.size().1
                );
                return Ok(current_mode);
            }
        }
    }
    // nVidia GPUs don't expose the encoder (and thus neither the crtc)
    log::warn!(
        "Could not detect current mode for display {:?},",
        connector.interface()
    );
    log::warn!("reading native resolution");
    return Ok(connector.modes()[0]);
}

/// Increase `log::LevelFilter` by one level
fn increment_loglevel(level: log::LevelFilter) -> log::LevelFilter {
    use log::LevelFilter::*;
    match level {
        Off => Error,
        Error => Warn,
        Warn => Info,
        Info => Debug,
        Debug => Trace,
        Trace => Trace,
    }
}

/// Decrease `log::LevelFilter` by one level
fn decrement_loglevel(level: log::LevelFilter) -> log::LevelFilter {
    use log::LevelFilter::*;
    match level {
        Off => Off,
        Error => Off,
        Warn => Error,
        Info => Warn,
        Debug => Info,
        Trace => Debug,
    }
}
