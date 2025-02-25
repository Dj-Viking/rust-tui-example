use spectrum_analyzer::FrequencySpectrum;

pub type VisualFunc = fn(f32, f32, f32, &FrequencySpectrum, f32) -> f32;

pub const SPIRAL_FUNC: VisualFunc = |y, x, t, _, _| y * x * t; 
pub const V2_FUNC:     VisualFunc = |y, x, t, _, _| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05);
pub const WAVES_FUNC:  VisualFunc = |y, x, t, _, _| x / y * t;
pub const SOLID_FUNC:  VisualFunc = |y, x, t, _, _| (x % 2.0 + 1000.0) / (y % 2.0 + 1000.0) * (t);

pub const AUDIO_FUNC:  VisualFunc = |y, x, t, fft_data, _| {
	// magnitudes are huge coming from fft_data
	// lets make it a usable number for our situation
	// can noise clamp be midi controllable?
	const NOISE_CLAMP: f32 = 10.0;
	const FREQ_AVERAGE: f32 = 500.0;
	const MAG_DIVISOR: f32 = 1000000.0;

	let mut xmod = 0.0;
	let mut ymod = 0.0;
	for (fr, fr_val) in fft_data.data().iter() {
		let freq = fr.val();
		let mag = fr_val.val() / MAG_DIVISOR;
		if freq >= FREQ_AVERAGE && mag > 0.0001 {
			xmod = mag;
			break;
		}
	}

	// can't get around the noise - not sure what to do with this yet
	if xmod < NOISE_CLAMP { xmod = 1.0 + xmod / 2.0 }
	println!("what is this thing {}", xmod);
	//println!("");
	//println!("what is this {}", t / 100.0);
	(y - xmod) * (x * xmod) * t / 100.0
};
