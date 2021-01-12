//! Sends DMX data, using an FTDI USB device via [`libftdi1_sys`].

extern crate libc;
extern crate libftdi1_sys as ftdic;
extern crate safe_ftdi as ftdi;

use libc::usleep;

/// Safely invokes a native ftdi function, preserving non-error return codes in an [`ftdi::Result<os::raw::c_int>`]
macro_rules! ftdi_try {
	($ftdi_fn:expr, $ftdi_context:expr, $($other_args:expr),*) => {
		unsafe {
			let ctx = $ftdi_context;
			let rc = $ftdi_fn(ctx, $($other_args,)*);
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

/// Trait to add missing [`ftdic`] methods to [`ftdi::Context`]
trait Context {
	/// Uses [`ftdic::ftdi_set_line_property2`] to set or unset the break signal
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

/// Sends DMX universe data to the default FTDI USB device
pub fn send(universe: [u8; 512]) -> ftdi::Result<()> {
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
