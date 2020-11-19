extern crate libc;
extern crate libftdi1_sys as ftdic;
extern crate safe_ftdi as ftdi;

use libc::usleep;
use std::env::args;
use std::os::raw::c_int;

fn ftdi_try(ftdi_context: *mut ftdic::ftdi_context, rc: c_int) -> ftdi::Result<c_int> {
	if rc < 0 {
		let slice = unsafe { std::ffi::CStr::from_ptr(ftdic::ftdi_get_error_string(ftdi_context)) };
		Err(ftdi::error::Error::LibFtdi(ftdi::error::LibFtdiError::new(slice.to_str().unwrap())))
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
		ftdi_try(ftdi_context, unsafe {
			ftdic::ftdi_set_line_property2(
				ftdi_context,
				ftdic::ftdi_bits_type::BITS_8,
				ftdic::ftdi_stopbits_type::STOP_BIT_2,
				ftdic::ftdi_parity_type::NONE,
				match on {
					true => ftdic::ftdi_break_type::BREAK_ON,
					false => ftdic::ftdi_break_type::BREAK_OFF,
				},
			)
		})?;
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

fn main() -> Result<(), String> {
	let mut universe: [u8; 512] = [0; 512];

	for arg in args().skip(1) {
		match arg.parse::<u16>() {
			Ok(n) => match n {
				0..=511 => universe[n as usize] = 200,
				_ => return Err("Args should be channel numbers".to_string()),
			},
			Err(_) => return Err("Args should be channel numbers".to_string()),
		};
	}

	match send(universe) {
		Ok(_) => (),
		Err(e) => return Err(e.to_string()),
	}

	Ok(())
}
