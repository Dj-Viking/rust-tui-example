use std::io::{BufReader, Read, Cursor};
use std::os::unix::net::UnixStream;
use std::ffi::CString;

use byteorder::ReadBytesExt;

use pulseaudio::protocol as ps;

const SAMPLES: usize = 256;
pub static mut SAMPLEBUF: [f32; SAMPLES] = [0.0; SAMPLES];

pub struct Audio {
	sock: BufReader<UnixStream>,
	sinf: ps::CreateRecordStreamReply,
	buf:  Vec<u8>,
}

impl Audio {
	pub fn init() -> Result<Self, ps::ProtocolError> {
		let socket_path = pulseaudio::socket_path_from_env().unwrap();
		let mut sock = BufReader::new(UnixStream::connect(socket_path).unwrap());

		let version = ps::MAX_VERSION;

		ps::write_command_message(
			sock.get_mut(), 0,
			ps::Command::Auth(ps::AuthParams {
				version,
				supports_shm:   false,
				supports_memfd: false,
				cookie: pulseaudio::cookie_path_from_env()
					.and_then(|p| std::fs::read(p).ok())
					.unwrap_or_default()
			}), version)?;

		let (_, auth_reply) = ps::read_reply_message::<ps::AuthReply>(&mut sock, version)?;

		let version = std::cmp::min(version, auth_reply.version);

		let mut props = ps::Props::new();
		props.set(ps::Prop::ApplicationName, CString::new("poop").unwrap());

		ps::write_command_message(sock.get_mut(), 1, ps::Command::SetClientName(props), version)?;

		let _ = ps::read_reply_message::<ps::SetClientNameReply>(&mut sock, version)?;


		// FIXME: hardcoded device name :P
		const DEVNAME: &str = "alsa_input.usb-Yamaha_Corporation_Yamaha_AG06MK2-00.analog-stereo";
		
		ps::write_command_message(
			sock.get_mut(),
			10,
			ps::Command::GetSourceInfo(ps::GetSourceInfo {
				name: Some(CString::new(DEVNAME).unwrap()),
				..Default::default()
			}),
			version)?;

		let (_, inf) = ps::read_reply_message::<ps::SourceInfo>(&mut sock, version)?;

		#[cfg(debug_assertions)]
		println!("audio socket {:#?}", sock);

		// make recording stream on the server
		ps::write_command_message(
			sock.get_mut(),
			99,
			ps::Command::CreateRecordStream(
				ps::RecordStreamParams {
					source_index: Some(inf.index),
					sample_spec: ps::SampleSpec {
						format:      inf.sample_spec.format,
						channels:    inf.channel_map.num_channels(),
						sample_rate: inf.sample_spec.sample_rate,
					},
					channel_map: inf.channel_map,
					cvolume:     Some(ps::ChannelVolume::norm(2)),
					..Default::default()
				}
			),
			version)?;

		let (_, sinf) = ps::read_reply_message::<ps::CreateRecordStreamReply>(&mut sock, version)?;
		println!("record strim reply {:#?}", sinf);

		Ok(Self {
			buf: vec![0; SAMPLES * 4],
			sock, sinf,
		})
	}

	pub fn read_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		// note: make sure pavucontrol is running to ensure that
		// pulseaudio is going to play nice and not have really bad
		// latency (around 2 seconds between reads if pavucontrol is off)
		self.sock.read_exact(&mut self.buf)?;

		let mut cursor = Cursor::new(self.buf.as_slice());
		let mut i = 0;

		while cursor.position() < cursor.get_ref().len() as u64 {
			unsafe { 
				SAMPLEBUF[i] = cursor.read_i32::<byteorder::LittleEndian>()? as f32; 
			}

			i += 1;
			if i >= unsafe { SAMPLEBUF.len() } { break; }
		}

		unsafe {
			if i < SAMPLEBUF.len() {
				(i..SAMPLEBUF.len()).for_each(|i| SAMPLEBUF[i] = 0.0);
			}
		}

		Ok(())
	}

	pub fn sample_rate(&self) -> u32 
	{ self.sinf.sample_spec.sample_rate }
}
