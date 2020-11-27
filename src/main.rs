extern crate libc;
extern crate libftdi1_sys as ftdic;
extern crate safe_ftdi as ftdi;

use libc::usleep;
use std::convert::TryInto;
use std::env::args;
use std::fs::File;
use std::io::{Error, ErrorKind};
use std::io::Write;
use std::os::raw::c_int;

const STATE_FILE_PATH: &str = "/root/dmx.dmxstate";

unsafe fn ftdi_try(ftdi_context: *mut ftdic::ftdi_context, rc: c_int) -> ftdi::Result<c_int> {
	if rc < 0 {
		let slice = std::ffi::CStr::from_ptr(ftdic::ftdi_get_error_string(ftdi_context));
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
		unsafe { ftdi_try(ftdi_context, ftdic::ftdi_set_line_property2(
			ftdi_context,
			ftdic::ftdi_bits_type::BITS_8,
			ftdic::ftdi_stopbits_type::STOP_BIT_2,
			ftdic::ftdi_parity_type::NONE,
			match on {
				true => ftdic::ftdi_break_type::BREAK_ON,
				false => ftdic::ftdi_break_type::BREAK_OFF,
			},
		))}?;
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
        Err(_) => Err(Error::new(ErrorKind::InvalidData, "State file is wrong length")),
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
        }
    }
}

fn main() -> Result<(), String> {
	let mut universe: [u8; 512] = match args().nth(1).and_then(|arg| arg.chars().nth(0)) {
        Some('-') | Some('+') | Some('^') => match read_state() {
            Ok(vec) => vec,
            Err(_) => { eprintln!("Couldn't read state file; turning everything off!"); [0; 512] },
        },
        _ => [0; 512],
    };
    let mut mode = Mode::Enable;

	for arg in args().skip(1) {
        let chan_number = match arg.chars().nth(0) {
            Some('-') => { mode = Mode::Disable; &arg[1..] },
            Some('+') => { mode = Mode::Enable; &arg[1..] },
            Some('^') => { mode = Mode::Toggle; &arg[1..] },
            _ => &arg[..],
        };

		match chan_number.parse::<u16>() {
			Ok(n) => match n {
				0..=511 => universe[n as usize] = new_value(&mode, universe[n as usize]),
				_ => return Err("Args should be channel numbers".to_string()),
			},
			Err(_) => return Err("Args should be channel numbers".to_string()),
		};
	}

    File::create(STATE_FILE_PATH).and_then(|mut f| f.write(&universe) ).expect("Failed to write state file");
	match send(universe) {
		Ok(_) => (),
		Err(e) => return Err(e.to_string()),
	}

	Ok(())
}
