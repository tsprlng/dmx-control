extern crate libc;
extern crate libftdi1_sys as ftdic;
extern crate safe_ftdi as ftdi;

use libc::usleep;
use std::convert::TryInto;
use std::env::args;
use std::fs::File;
use std::io::Write;
use std::io::{Error, ErrorKind};
use std::os::raw::c_int;

const STATE_FILE_PATH: &str = "/root/dmx.dmxstate";

unsafe fn ftdi_try(ftdi_context: *mut ftdic::ftdi_context, rc: c_int) -> ftdi::Result<c_int> {
	if rc < 0 {
		let slice = std::ffi::CStr::from_ptr(ftdic::ftdi_get_error_string(ftdi_context));
		Err(ftdi::error::Error::LibFtdi(ftdi::error::LibFtdiError::new(
			slice.to_str().unwrap(),
		)))
	} else {
		Ok(rc)
	}
}

trait Context {
	fn set_break(&self, on: bool) -> ftdi::Result<()>;
}
impl Context for ftdi::Context {
	fn set_break(&self, on: bool) -> ftdi::Result<()> {
		let ftdi_context = self.get_ftdi_context();
		unsafe {
			ftdi_try(
				ftdi_context,
				ftdic::ftdi_set_line_property2(
					ftdi_context,
					ftdic::ftdi_bits_type::BITS_8,
					ftdic::ftdi_stopbits_type::STOP_BIT_2,
					ftdic::ftdi_parity_type::NONE,
					match on {
						true => ftdic::ftdi_break_type::BREAK_ON,
						false => ftdic::ftdi_break_type::BREAK_OFF,
					},
				),
			)
		}?;
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
		unsafe { usleep(10000) };
		context.set_break(false)?;
		unsafe { usleep(8) };

		context.write_data(&universe)?;
		unsafe { usleep(15000) };
	}
	Ok(())
}

fn read_state() -> std::io::Result<[u8; 512]> {
	let v = std::fs::read(STATE_FILE_PATH)?;
	match v.try_into() {
		Ok(arr) => Ok(arr),
		Err(_) => Err(Error::new(
			ErrorKind::InvalidData,
			"State file is wrong length",
		)),
	}
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

const ARG_ERROR: &str = "Args should be channel numbers";

fn parse_arg(arg: &String) -> Result<(Option<Mode>, u16), String> {
	let (mode, chan_number) = match arg.chars().nth(0) {
		Some('-') => (Some(Mode::Disable), &arg[1..]),
		Some('+') => (Some(Mode::Enable), &arg[1..]),
		Some('^') => (Some(Mode::Toggle), &arg[1..]),
		_ => (None, &arg[..]),
	};

	match chan_number.parse::<u16>() {
		Ok(n) => match n {
			0..=511 => Ok((mode, n)),
			_ => Err(ARG_ERROR.to_string()),
		},
		Err(_) => Err(ARG_ERROR.to_string()),
	}
}

fn main() -> Result<(), String> {
	let mut universe: [u8; 512] = match args().nth(1).map(|a| parse_arg(&a)).transpose()? {
		Some((Some(_), _)) => match read_state() {
			Ok(vec) => vec,
			Err(_) => {
				eprintln!("Couldn't read state file; turning everything off!");
				[0; 512]
			}
		},
		_ => [0; 512],
	};

	let mut mode = Mode::Enable;

	for arg in args().skip(1) {
		let (new_mode, chan_number) = parse_arg(&arg)?;
		match new_mode {
			Some(m) => mode = m,
			_ => (),
		}
		universe[chan_number as usize] = new_value(&mode, universe[chan_number as usize]);
	}

	match send(universe) {
		Ok(_) => (),
		Err(e) => return Err(e.to_string()),
	}
	File::create(STATE_FILE_PATH)
		.and_then(|mut f| f.write(&universe))
		.or(Err("Failed to write state file"))?;

	Ok(())
}
