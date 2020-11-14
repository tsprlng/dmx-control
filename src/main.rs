extern crate libc;
extern crate libftdi1_sys as ftdic;
extern crate safe_ftdi as ftdi;

use libc::usleep;
use std::env::args;

trait Context {
	fn set_break(&self, on: bool) -> ftdi::Result<()>;
}
impl Context for ftdi::Context {
	fn set_break(&self, on: bool) -> ftdi::Result<()> {
		let rc = unsafe {
			ftdic::ftdi_set_line_property2(
				self.context,
				ftdic::ftdi_bits_type::BITS_8,
				ftdic::ftdi_stopbits_type::STOP_BIT_2,
				ftdic::ftdi_parity_type::NONE,
				match on {
					true => ftdic::ftdi_break_type::BREAK_ON,
					false => ftdic::ftdi_break_type::BREAK_OFF,
				},
			)
		};
		self.check_ftdi_error(rc, ())
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
