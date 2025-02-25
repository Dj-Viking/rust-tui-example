use portmidi::{DeviceInfo, MidiEvent};
use std::collections::HashMap;

use crate::{DConfig, MutState, ActiveFunc};

pub struct Midi {
	pub dev: DeviceInfo,
	pub cfg: DConfig,
}

impl Midi {
	pub fn new(pm_ctx: &portmidi::PortMidi) -> Result<Self, Box<dyn std::error::Error>> {
		let devices = pm_ctx.devices()?;
		let mut config: HashMap<String, DConfig> = 
			toml::from_str(&std::fs::read_to_string(crate::CONF_FILE).unwrap()).unwrap_or_else(|e| {
				eprintln!("Error reading config file: {e}");
				std::process::exit(1);
			});


		let dev = devices.into_iter()
			.find(|d| 
				d.direction() == portmidi::Direction::Input 
				&& config.keys().any(|n| n == d.name()))
			.unwrap_or_else(|| {
				eprintln!("No device defined in config found");
				std::process::exit(1);
			});

		Ok(Self {
			cfg: unsafe { config.remove(dev.name()).unwrap_unchecked() },
			dev,
		})
	}

	pub fn handle_msg(&self, me: MidiEvent, ms: &mut MutState) {

		let channel   = me.message.data1;
		let intensity = me.message.data2;

		#[cfg(debug_assertions)]
		println!("chan {} - intensity {}", channel, intensity);

		let max_intensity_range = match ms.func {
			ActiveFunc::Solid | ActiveFunc::Waves => 100.0,
			_ => 0.01,
		};

		match channel {
			c if c == self.cfg.backwards => ms.is_backwards = !ms.is_backwards,

			c if c == self.cfg.intensity      => ms.current_intensity = lerp_float(intensity, 0.0, max_intensity_range, 0, 127),
			c if c == self.cfg.decay_factor   => ms.decay_factor = lerp_float(intensity, 0.0, 1.0, 0, 127),
			c if c == self.cfg.time_dialation => ms.time_dialation = lerp_float(intensity, 0.0, 100.0, 0, 127),
			_ if intensity == 0          => (),

			c if c == self.cfg.spiral    => ms.func = ActiveFunc::Spiral,
			c if c == self.cfg.v2        => ms.func = ActiveFunc::V2,
			c if c == self.cfg.waves     => ms.func = ActiveFunc::Waves,
			c if c == self.cfg.solid     => ms.func = ActiveFunc::Solid,
			c if c == self.cfg.audio     => ms.func = ActiveFunc::Audio,

			_ => (),
		}

		ms.is_reset = channel == self.cfg.reset && intensity > 0;
	}
}

// output a number within a specific range from an entirely 
pub fn lerp_float(
    input:      u8,  // - input value to determine what position in the range the number sits
    min_output: f32, // - minimum percentage value
    max_output: f32, // - maximum percentage value
    min_input:  u8,  // - minimum input value the range can be
    max_input:  u8,  // - maximum input value the range can be
) -> f32 {

	let val = (max_output - min_output) * ((input - min_input) as f32)
	   / ((max_input - min_input) as f32) + min_output;
	println!("lerp float {}", val);
	val
}
