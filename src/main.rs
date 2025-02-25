#![allow(static_mut_refs)]

use portmidi::PortMidi;
use nannou::prelude::*;

use spectrum_analyzer::{
	FrequencySpectrum, 
	samples_fft_to_spectrum, 
	FrequencyLimit,
	windows::hann_window,
	scaling::divide_by_N_sqrt
};

use std::sync::{Arc, Mutex};

mod audio;
mod midi;
mod funcs;

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
	#[allow(clippy::type_complexity)]
	funcs:       &'static [funcs::VisualFunc],
	ms:          Arc<Mutex<MutState>>,
	sample_rate: u32,
}

#[derive(Default)]
struct MutState {
	is_backwards:      bool,
	is_reset:          bool,
	current_intensity: f32,
	time_dialation:    f32,
	decay_factor:      f32,
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
	decay_factor:   u8,
	reset:          u8,
}

const CONF_FILE: &str = "config.toml";

fn main() {
	
	// list midi devices in terminal
	if std::env::args().skip(1).any(|a| a == "list") {
		let pm_ctx = PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();
		devices.iter().for_each(|d| println!("{} {:?} {:?}", d.id(), d.name(), d.direction()));
		std::process::exit(0);
	}

	// init as hot reloadable??
	let init = |a: &App| { 

		let ms = Arc::new(Mutex::new(MutState::default()));
		let ms_ = ms.clone();

			
		let pm_ctx = PortMidi::new().unwrap();
		let midi = midi::Midi::new(&pm_ctx).unwrap();


		let mut audio = audio::Audio::init().unwrap();
		let sample_rate = audio.sample_rate();

		// audio stream thread
		std::thread::spawn(move || {
			loop {
				std::thread::sleep(std::time::Duration::from_millis(1));
				audio.read_stream().unwrap()
			}
		});

		if !std::env::args().skip(1).any(|a| a == "keys") {

			std::thread::spawn(move || {

				let mut in_port = pm_ctx.input_port(midi.dev.clone(), 256).unwrap();

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

					let mut ms = ms_.lock().unwrap();

					midi.handle_msg(m, &mut ms);

					unsafe { BACKOFF = 0; }
				}
			});
		}

		a.new_window()
			.view(view)
			.key_pressed(key_pressed)
			.key_released(key_released)
			.build().unwrap(); 


		State {
			ms, sample_rate,
			funcs: &[
				funcs::SPIRAL_FUNC,
				funcs::V2_FUNC,
				funcs::WAVES_FUNC,
				funcs::SOLID_FUNC,
				funcs::AUDIO_FUNC
			],
		}
	};

	nannou::app(init).run();
}

fn key_released(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();
	match key {
		Key::R => ms.is_reset = false,
		_ => ()
	}
}

fn key_pressed(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();
	match key {
		Key::R => ms.is_reset = true,

		Key::S => ms.func = ActiveFunc::Spiral,
		Key::W => ms.func = ActiveFunc::Waves,
		Key::O => ms.func = ActiveFunc::Solid,
		Key::V => ms.func = ActiveFunc::V2,
		Key::A => ms.func = ActiveFunc::Audio,

		Key::Up    if ms.current_intensity < 255.0 => ms.current_intensity += 1.0,
		Key::Down  if ms.current_intensity > 0.0   => ms.current_intensity -= 1.0,
		Key::Right if ms.time_dialation    < 255.0 => ms.time_dialation += 1.0,
		Key::Left  if ms.time_dialation    > 0.0   => ms.time_dialation -= 1.0,
 
		_ => (),
	}
}

fn view(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();

	// nice
	let mut fft_buf = [0.0; 69];

	let fft = samples_fft_to_spectrum(
		&hann_window(unsafe { &audio::SAMPLEBUF }),
		s.sample_rate,
		FrequencyLimit::Range(50.0, 12000.0),
		Some(&divide_by_N_sqrt)
	).unwrap();

	let fr_mags: Vec<(f32, f32)> = fft.data().iter().map(|(fr, mag)| (fr.val(), mag.val())).collect();

	// a pretty good decay factor
	// controlled by midi but here for reference
	// gives a slow smeary like feeling
	const FACTOR: f32 = 0.9999;

	fr_mags.iter().map(|(_, x)| x)
		.zip(fft_buf.iter_mut()).for_each(|(c, p)| 
			if *c > *p { *p = *c; } 
			//else { *p *= ms.decay_factor; });
			else { *p *= FACTOR; });

	//let fr_mags = exponential_moving_average(&fr_mags, 0.2);

	//println!("length of fft data stuff {}", fft_data.data().iter().len());
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
			(time_divisor + 100000.0 * (ms.time_dialation / 10.0))
			+ ms.current_intensity / 100.0;

		let val = s.funcs[ms.func as u8 as usize](r.y(), r.x(), t, &fft, time_divisor);

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(val, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}

// fn exponential_moving_average(data: &[(f32, f32)], alpha: f32) -> Vec<(f32, f32)> {
//     let mut smoothed = Vec::with_capacity(data.len());
//     let mut prev = data[0];
//     for &(fr, val) in data {
//         let smoothed_val = alpha * val + (1.0 - alpha) * prev.1;
//         smoothed.push((fr, smoothed_val));
//         prev = (fr, smoothed_val);
//     }
//     smoothed
// }
