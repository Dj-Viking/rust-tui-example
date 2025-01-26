use portmidi::PortMidi;
use nannou::prelude::*;

use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{
	FrequencySpectrum, 
	samples_fft_to_spectrum, 
	FrequencyLimit
};
use spectrum_analyzer::scaling::divide_by_N_sqrt;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

mod audio;

#[derive(Debug, Clone, PartialEq, Copy, Default)]
#[repr(u8)]
enum ActiveFunc {
	#[default]
	Spiral = 0,
	V2     = 1,
	Waves  = 2,
	Solid  = 3,
	Audio  = 4,
}

struct State {
	funcs:       &'static [fn(f32, f32, f32, &FrequencySpectrum) -> f32],
	ms:          Arc<Mutex<MutState>>,
	sample_rate: u32,
}

#[derive(Default)]
struct MutState {
	is_backwards:      bool,
	is_reset:          bool,
	current_intensity: u8,
	time_dialiation:   u8,
	func:              ActiveFunc,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct DConfig {
	backwards:      u8,
	v2:             u8,
	waves:          u8,
	solid:          u8,
	audio:          u8,
	spiral:         u8,
	intensity:      u8,
	time_dialation: u8,
	reset:          u8,
}

const CONF_FILE: &str = "config.toml";

fn main() {
	if std::env::args().skip(1).any(|a| a == "list") {
		let pm_ctx = PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();
		devices.iter().for_each(|d| println!("{} {:?} {:?}", d.id(), d.name(), d.direction()));
		std::process::exit(0);
	}

	let init = |a: &App| { 
		//unsafe { AUDIO_STATE = vec![0.0; 256]; }

		let ms = Arc::new(Mutex::new(MutState::default()));
		let ms_ = ms.clone();

		let pm_ctx = PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();

		let mut audio = audio::Audio::init().unwrap();
		let sample_rate = audio.sample_rate();

		std::thread::spawn(move || {
			loop {
				std::thread::sleep(std::time::Duration::from_millis(1));
				audio.read_stream().unwrap()
			}
		});

		let (cfg, dev) = {
			let mut config: HashMap<String, DConfig> = 
				toml::from_str(&std::fs::read_to_string(CONF_FILE).unwrap()).unwrap_or_else(|e| {
					eprintln!("Error reading config file: {e}");
					std::process::exit(1);
				});

			let dev = devices.into_iter()
				.find(|d| d.direction() == portmidi::Direction::Input && config.keys().any(|n| n == d.name()))
				.unwrap_or_else(|| {
					eprintln!("No device defined in config found");
					std::process::exit(1);
				});

			(unsafe { config.remove(dev.name()).unwrap_unchecked() }, dev)
		};

		std::thread::spawn(move || {
			let mut in_port = pm_ctx.input_port(dev, 256).unwrap();

			loop {
				static mut BACKOFF: u8 = 0;
				// TODO: listen flag

				let Ok(Some(m)) = in_port.read() else {
					std::hint::spin_loop();

					std::thread::sleep(
						std::time::Duration::from_millis(
							unsafe { BACKOFF * 10 } as u64
						)
					);

					unsafe { BACKOFF += 1; }
					unsafe { BACKOFF %= 10; }
					continue;
				};

				let channel   = m.message.data1;
				let intensity = m.message.data2;

				#[cfg(debug_assertions)]
				println!("chan {} - intensity {}", channel, intensity);

				let mut ms = ms_.lock().unwrap();

				match channel {
					c if c == cfg.intensity => ms.current_intensity = intensity,
					c if c == cfg.time_dialation => ms.time_dialiation = intensity,
					_ if intensity == 0     => (),
					c if c == cfg.spiral    => ms.func = ActiveFunc::Spiral,
					c if c == cfg.v2        => ms.func = ActiveFunc::V2,
					c if c == cfg.waves     => ms.func = ActiveFunc::Waves,
					c if c == cfg.solid     => ms.func = ActiveFunc::Solid,
					c if c == cfg.audio     => ms.func = ActiveFunc::Audio,
					c if c == cfg.backwards => ms.is_backwards = !ms.is_backwards,
					_ => (),
				}

				ms.is_reset = channel == cfg.reset && intensity > 0;

				unsafe { BACKOFF = 0; }
			}
		});

		a.new_window()
			.view(view)
			.build().unwrap();

		State {
			ms, sample_rate,
			funcs: &[
				|y, x, t, _| y * x * t, // spiral
				|y, x, t, _| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05), // v2
				|y, x, t, _| x / y * t, // waves
				|y, x, t, _| (x % 2.0 + 1000.0) / (y % 2.0 + 1000.0) * (t), // solid
				|y, x, mut t, fft_data| { // audio
					// what to do here??
					// the app got a lot slower now :( but maybe on the right track?
					for (fr, fr_val) in fft_data.data().iter() {
						if fr.val() < 500.0 {
							if fr_val.val() > 100.0 { t += 100.0; }
						} else {

						}
					}
					y * x * t
				}
			],
		}
	};

	nannou::app(init).run();
}

fn view(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();

	let fft = samples_fft_to_spectrum(
		&hann_window(unsafe { &audio::SAMPLEBUF }),
		s.sample_rate,
		FrequencyLimit::Range(50.0, 12000.0),
		Some(&divide_by_N_sqrt)
	).unwrap();

	static mut TIME: f32 = 0.0;

	let time_divisor = match ms.func {
		ActiveFunc::Waves | ActiveFunc::Solid => 1000.0,
		_ => 1000000000.0,
	};

	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter()) {

		match ms.is_backwards {
			true => unsafe { TIME -= app.duration.since_prev_update.as_secs_f32() },
			_    => unsafe { TIME += app.duration.since_prev_update.as_secs_f32() },
		}

		const THRESHOLD: f32 = 1000000000.0;
		if unsafe { TIME >= THRESHOLD || TIME <= -THRESHOLD } {
			ms.is_backwards = !ms.is_backwards;
		}

		if ms.is_reset { unsafe { TIME = 0.0; } } 
		
		let t = unsafe { TIME } /
			(time_divisor + 100000.0 * (ms.time_dialiation as f32 / 10.0))
			+ ms.current_intensity as f32 / 100.0;

		let val = s.funcs[ms.func as u8 as usize](r.y(), r.x(), t, &fft);

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(val, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}
