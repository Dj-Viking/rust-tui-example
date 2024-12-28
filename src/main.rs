use portmidi as pm;
use nannou::prelude::*;
use std::time::Duration;
use std::thread;
use std::collections::HashMap;

static mut TIME_NOW: f32 = 0.0;
const TIME_DIVISOR: f32 = 1000000000.0;

const SPIRAL_FN: EffectFn = |y, x, t| y * x * t;

const V2_FN: EffectFn = |y, x, t| {
	 32.0 / 
		(t / x) + y / 
		(x / y - 1.0 / t) +
		t * (y * 0.05)
};

#[derive(Debug)]
enum ActiveFunc {
	Spiral,
	V2
}

#[derive(Debug)]
struct MidiState {
	current_channel: u8,
	current_intensity: u8,

	intensity_channel: u8,
	time_dialation_channel: u8,

	func_on: ActiveFunc,

	reset_channel: u8,
	is_reset: bool,

	v2_channel: u8,

	spiral_channel: u8,

	timeout: Duration,
}

const fn new_midi_state() -> MidiState {
	MidiState {
		current_channel: 0,
		current_intensity: 0,

		time_dialation_channel: 0,
		intensity_channel: 0,

		func_on: ActiveFunc::Spiral,

		reset_channel: 0,
		is_reset: false,

		v2_channel: 0,

		spiral_channel: 0,

		timeout: Duration::from_millis(10),
	}
}

static mut MS: MidiState = new_midi_state();

type EffectFn = fn(f32, f32, f32) -> f32;


struct State {
	finx:  usize,
	reset: bool,
	funcs: Vec<EffectFn>,
	funcmap: HashMap<u8, EffectFn>
}

// TODO: figure out how to dynamically get the controller I want to use
// this could be better
// from the config file and all it's mappings
// for now only mapped up to XONE controller
// the format is hard coded for now
fn read_midi_input_config() -> () {
	let parts = std::fs::read_to_string(".midi-input-config").unwrap()
		.split('\n')
		.filter(|l| !l.is_empty() && !l.contains("#"))
		.map(|l| l.to_string())
		.collect::<Vec<String>>();

	for i in 0..parts.len() {
		println!("{}", parts[i]);
		// hard coded known format only 
		// two entries below the XONE label in the config file for now
		if parts[i].contains("[XONE]") {
			unsafe {
				MS.intensity_channel = parts[i + 1]
					.split('=')
					.collect::<Vec<&str>>()[1]
					.parse::<u8>().unwrap();
				MS.time_dialation_channel = parts[i + 2]
					.split('=')
					.collect::<Vec<&str>>()[1]
					.parse::<u8>().unwrap();
				MS.reset_channel = parts[i + 3]
					.split('=')
					.collect::<Vec<&str>>()[1]
					.parse::<u8>().unwrap();
				MS.spiral_channel = parts[i + 4]
					.split('=')
					.collect::<Vec<&str>>()[1]
					.parse::<u8>().unwrap();
				MS.v2_channel = parts[i + 5]
					.split('=')
					.collect::<Vec<&str>>()[1]
					.parse::<u8>().unwrap();
			}
		}
	}
}


fn main() {

	let init = |a: &App| { 

		let pm_ctx = pm::PortMidi::new().unwrap();
		let xone_id = get_xonek2_input_id(&pm_ctx);
		let info = pm_ctx.device(xone_id).unwrap();

		// map the channels to which part of the effect to control
		read_midi_input_config();

		thread::spawn(move || {
			let in_port = pm_ctx.input_port(info, 1024)
				.unwrap();
			while let Ok(_) = in_port.poll() {
				if let Ok(Some(m)) = in_port.read_n(1024) {
					handle_midi_msg(MyMidiMessage::new(m[0]));
				}
			}
			unsafe {
				thread::sleep(MS.timeout);
			}
		});


		a.new_window()
			.view(update)
			.key_pressed(key_pressed)
			.key_released(key_released)
			.build().unwrap(); 
	
		unsafe {
			let hm = HashMap::from([
				(MS.spiral_channel, SPIRAL_FN),
				(MS.v2_channel, V2_FN)
			]);

			State {
				finx: 0,
				reset: false,
				funcs: vec![SPIRAL_FN, V2_FN],
				funcmap: hm
			}
		}
	};

	nannou::app(init).run();
}

#[derive(Debug)]
struct MyMidiMessage {
	channel: u8,
	intensity: u8,
}
impl MyMidiMessage {
	fn new(m: pm::types::MidiEvent) -> Self {
		Self {
			channel: m.message.data1,
			intensity: m.message.data2,
		}
	}
}

fn handle_midi_msg(m: MyMidiMessage) -> () {
	unsafe {
		MS.current_channel = m.channel;

		if listen_midi_channel(
			m.channel, MS.intensity_channel) 
		{
			MS.current_intensity = m.intensity;
		}

		if listen_midi_channel(
			m.channel, MS.reset_channel) 
		{
			if m.intensity == 127 {
				MS.is_reset = true;
			} else {
				MS.is_reset = false;
			}
		}
		if listen_midi_channel(
			m.channel, MS.spiral_channel) 
		{
			if m.intensity == 127 {
				MS.func_on = ActiveFunc::Spiral;
			}
		}
		if listen_midi_channel(
			m.channel, MS.v2_channel) 
		{
			if m.intensity == 127 {
				MS.func_on = ActiveFunc::V2;
			} 
		}

	}
}

fn get_xonek2_input_id(pm: &pm::PortMidi) -> i32 {
	let mut ret: i32 = 0;

	for d in pm.devices().unwrap()
	{
		if d.name().contains("XONE") 
			&& d.direction() == pm::Direction::Input
		{ ret = d.id(); break; }
	}

	ret
}

fn key_released(_: &App, s: &mut State, key: Key) {
	match key {
		Key::Tab => s.reset = false,
		_ => (),
	}
}
fn key_pressed(_: &App, s: &mut State, key: Key) {
	match key {
		Key::Space => s.finx = (s.finx + 1) % s.funcs.len(),
		Key::Tab => s.reset = true,
		_ => (),
	}
}

fn listen_midi_channel(in_channel: u8, channel: u8) -> bool {
	if in_channel == channel {
		return true;
	}
	return false;
}

fn update(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);

	let f3 = |s: &State| {
		unsafe {
			match &MS.func_on {
				ActiveFunc::Spiral => s.funcmap[&MS.spiral_channel],
				ActiveFunc::V2 => s.funcmap[&MS.v2_channel],
				// default to spiral if haven't handled that func_on value yet
				_ => s.funcs[0],
			}
		}
	};

	let t = || {
		unsafe {
			TIME_NOW += app.duration.since_prev_update.as_secs_f32();
			if MS.is_reset {
				TIME_NOW = 0.0; 
			}
			return TIME_NOW / TIME_DIVISOR + (MS.current_intensity as f32 / 100.0) as f32;
		}
	};

	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
	{
		let hue = f3(s)(r.y(), r.x(), t());

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}
