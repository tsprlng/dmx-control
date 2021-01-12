extern crate dirs;
extern crate libc;
extern crate libftdi1_sys as ftdic;
extern crate safe_ftdi as ftdi;

use {
	libc::usleep,
	std::{
		convert::TryInto,
		io::{Error, ErrorKind},
		path::{Path, PathBuf},
	},
};

fn state_file_path() -> std::io::Result<Box<Path>> {
	if let Ok(path_str) = std::env::var("DMX_STATE_PATH") {
		return Ok(PathBuf::from(path_str).into_boxed_path());
	}
	if let Some(mut home_path) = dirs::home_dir() {
		home_path.push(".cache");
		if home_path.is_dir() {
			home_path.push("dmx.state");
			return Ok(home_path.into_boxed_path());
		}
	}
	Err(Error::new(ErrorKind::NotFound, "State file can't be found"))
}

macro_rules! ftdi_try {
	($ftdi_fn:expr, $ctx:expr, $($a:expr),*) => {
		unsafe {
			let ctx = $ctx;
			let rc = $ftdi_fn(ctx, $($a,)*);
			if rc < 0 {
				let slice = std::ffi::CStr::from_ptr(ftdic::ftdi_get_error_string(ctx));
				Err(ftdi::error::Error::LibFtdi(ftdi::error::LibFtdiError::new(
					slice.to_str().unwrap()
				)))
			} else {
				Ok(rc)
			}
		}
	};
}

trait Context {
	fn set_break(&self, on: bool) -> ftdi::Result<()>;
}
impl Context for ftdi::Context {
	fn set_break(&self, on: bool) -> ftdi::Result<()> {
		ftdi_try!(
			ftdic::ftdi_set_line_property2,
			self.get_ftdi_context(),
			ftdic::ftdi_bits_type::BITS_8,
			ftdic::ftdi_stopbits_type::STOP_BIT_2,
			ftdic::ftdi_parity_type::NONE,
			match on {
				true => ftdic::ftdi_break_type::BREAK_ON,
				false => ftdic::ftdi_break_type::BREAK_OFF,
			}
		)?;
		Ok(())
	}
}

fn send(universe: [u8; 512]) -> ftdi::Result<()> {
	let mut context = ftdi::Context::new()?;
	context.open(0x0403, 0x6001)?;
	context.set_baudrate(250_000)?;

	for _ in 0..10 {
		// TODO repeating transmission is enough -- it works -- but why is it unreliable in the first place?

		context.set_break(true)?;
		unsafe { usleep(10_000) };
		context.set_break(false)?;
		unsafe { usleep(8) };

		context.write_data(&universe)?;
		unsafe { usleep(15_000) };
	}
	Ok(())
}

fn read_state() -> std::io::Result<[u8; 512]> {
	let state = std::fs::read(state_file_path()?)?;
	state.try_into().or(Err(Error::new(
		ErrorKind::InvalidData,
		"State file is wrong length",
	)))
}

fn write_state(universe: [u8; 512]) -> Result<(), Error> {
	std::fs::write(state_file_path()?, &universe)
}

enum Mode {
	Enable,
	Disable,
	Toggle,
}

fn new_value(m: &Mode, current_value: u8) -> u8 {
	match m {
		Mode::Enable => 200,
		Mode::Disable => 0,
		Mode::Toggle => match current_value {
			0 => 200,
			_ => 0,
		},
	}
}

fn parse_arg(arg: &String) -> Result<(Option<Mode>, u16), String> {
	let (mode, chan_number) = match arg.chars().nth(0) {
		Some('-') => (Some(Mode::Disable), &arg[1..]),
		Some('+') => (Some(Mode::Enable), &arg[1..]),
		Some('^') => (Some(Mode::Toggle), &arg[1..]),
		_ => (None, &arg[..]),
	};

	if let Ok(n) = chan_number.parse::<u16>() {
		if n < 512 {
			return Ok((mode, n));
		}
	};
	Err("Args should be channel numbers".to_string())
}

fn main() -> Result<(), String> {
	let args = (std::env::args()
		.skip(1)
		.map(|a| parse_arg(&a))
		.collect::<Result<Vec<_>, _>>())?;

	let is_stateful_request = args.iter().any(|(maybe_mode, _)| maybe_mode.is_some());
	let mut universe: [u8; 512] = match is_stateful_request {
		true => read_state().unwrap_or_else(|_| {
			eprintln!("Couldn't read state file; turning unspecified channels off!");
			[0; 512]
		}),
		false => [0; 512],
	};

	let mut mode = Mode::Enable;
	for (new_mode, chan_number) in args {
		if let Some(m) = new_mode {
			mode = m
		}
		universe[chan_number as usize] = new_value(&mode, universe[chan_number as usize]);
	}

	send(universe).map_err(|err| err.to_string())?;
	write_state(universe).or(Err("Failed to write state file"))?;

	Ok(())
}
