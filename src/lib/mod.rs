use core::fmt;
use core::time::Duration;
use serialport::SerialPortSettings;
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};

pub use serialport;

#[derive(Debug)]
pub enum Error {
    WrongDevice,
    Disconnected,
    UnsupportedVersion(u16),
    InvalidSpeed,
    Open(serialport::Error),
    Other(io::Error),
}

#[allow(non_camel_case_types)]
mod ffi {
    use crate::Error;
    use core::ptr;
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int};
    use std::sync::mpsc::Sender;
    use std::thread::JoinHandle;

    #[repr(C)]
    pub struct error_t {
        tag: error_type,
        payload: c_int,
    }

    #[repr(C)]
    pub enum error_type {
        OK,
        WRONG_DEVICE,
        DISCONNECTED,
        UNSUPPORTED_VERSION,
        INVALID_SPEED,
        OPEN,
        COMMUNICATION,
    }

    impl error_t {
        fn ok() -> Self {
            Self {
                tag: error_type::OK,
                payload: 0,
            }
        }
    }

    pub struct handle_t(JoinHandle<Result<(), Error>>);
    pub struct sender_t(Sender<i16>);

    fn convert_err(e: Error) -> error_t {
        fn err(t: error_type) -> error_t {
            error_t { tag: t, payload: 0 }
        }

        fn err_p(t: error_type, p: c_int) -> error_t {
            error_t { tag: t, payload: p }
        }

        match e {
            Error::WrongDevice => err(error_type::WRONG_DEVICE),
            Error::Disconnected => err(error_type::DISCONNECTED),
            Error::UnsupportedVersion(v) => err_p(error_type::UNSUPPORTED_VERSION, v as _),
            Error::InvalidSpeed => err(error_type::INVALID_SPEED),
            Error::Open(_) => err(error_type::OPEN), // TODO: Add payload
            Error::Other(e) => err_p(error_type::COMMUNICATION, e.raw_os_error().unwrap_or(0)),
        }
    }

    /// Start communication with the device
    /// This spawns a background thread
    ///
    /// Arguments:
    ///   serial: the path of the serial device (ex: '/dev/ttyACM0' on linux). Should be valid UTF8
    ///     and should be properly null-terminated
    ///   sender: the pointer to the sender will be written to this field.
    ///   handle: the pointer to the background thread handle, used to check for errors.
    ///
    /// If this function returns something other than OK, the sender reference is not valid. Else,
    ///   it must be released before exit using smov_drop_communication.
    #[no_mangle]
    pub unsafe extern "C" fn smov_connect(
        serial: *const c_char,
        sender: *mut *mut sender_t,
        handle: *mut *mut handle_t,
    ) -> error_t {
        match super::communicate(CStr::from_ptr(serial).to_str().unwrap()) {
            Ok((s, h)) => {
                *sender = Box::into_raw(Box::new(sender_t(s)));
                *handle = Box::into_raw(Box::new(handle_t(h)));
                error_t::ok()
            }
            Err(e) => convert_err(e),
        }
    }

    /// Sends a speed control to the listening micro-controller
    ///
    /// Arguments:
    ///   sender: the pointer to the sender reference handed out by smov_connect. Should
    ///     point to a non-null, valid reference
    ///   val: the speed command to send to the motor
    ///
    /// Return:
    ///   true: Ok
    ///   false: The connection was lost. Use smov_get_error to grab the precise error.
    ///            From this point on, the connection is no longer valid and a new connection
    ///            must be made using smov_connect if the program wishes to connect again
    #[no_mangle]
    pub unsafe extern "C" fn smov_set_speed(sender: *const sender_t, val: i16) -> bool {
        { &*sender }.0.send(val).is_ok()
    }

    /// List all the devices available for use with the library
    ///
    /// Each var is only the path of the device and can be handed in to smov_connect directly
    ///
    /// The caller is responsible to clean the memory, probably using smov_free_devices.
    #[no_mangle]
    pub unsafe extern "C" fn smov_list_devices() -> *mut *mut c_char {
        if let Ok(out) = serialport::available_ports() {
            let vec: Box<[_]> = out
                .into_iter()
                .map(|port| CString::new(port.port_name).unwrap().into_raw())
                .chain(core::iter::once(ptr::null_mut()))
                .collect();
            { &mut *Box::into_raw(vec) }.as_mut_ptr()
        } else {
            ptr::null_mut()
        }
    }

    /// Free the device list
    #[no_mangle]
    pub unsafe extern "C" fn smov_free_devices(devices: *mut *mut c_char) {
        let mut dev_count = 0;
        while *devices.add(dev_count) != ptr::null_mut() {
            core::mem::drop(CString::from_raw(*devices));
            dev_count += 1;
        }
        let devs = core::slice::from_raw_parts_mut(devices, dev_count);
        core::mem::drop(Box::from_raw(devs));
    }

    /// Get the exit status of the communication
    /// This call is blocking and returns only when the communication with the device stops
    ///
    /// Arguments:
    ///   handle: the pointer to the handle reference handed out by smov_connect. Should
    ///     point to a non-null, valid reference. This call consumes the handle and as
    ///     such, it should no longer be used
    ///
    /// Return:
    ///   The reason the communication stopped
    #[no_mangle]
    pub unsafe extern "C" fn smov_get_error(handle: *const handle_t) -> error_t {
        match handle.read().0.join() {
            Ok(Ok(())) => error_t::ok(),
            Ok(Err(e)) => convert_err(e),
            Err(_) => error_t {
                tag: error_type::COMMUNICATION,
                payload: 0,
            },
        }
    }

    /// Convert an error code to a string representation
    #[no_mangle]
    pub unsafe extern "C" fn smov_strerror(error: error_t) -> *const c_char {
        use error_type::*;
        match error.tag {
            OK => "Everything ok\0",
            WRONG_DEVICE => {
                "Connected to a device that does not respect the protocol. Try a reset?\0"
            }
            DISCONNECTED => "The device disconnected unexpectedly\0",
            UNSUPPORTED_VERSION => "The device has an unsupported version\0",
            INVALID_SPEED => "Could not set the speed on the device\0",
            OPEN => "Could not open the device\0",
            COMMUNICATION => "Communication failure\0",
        }
        .as_ptr() as _
    }

    /// Stop communication with the device
    /// This closes the communication with the tty and make the background thread return OK
    ///
    /// Arguments:
    ///   sender: the pointer to the sender reference handed out by smov_connect. Should
    ///     point to a valid reference. This call consumes the handle and, as such, the
    ///     sender must not be used again.
    #[no_mangle]
    pub unsafe extern "C" fn smov_drop_communication(sender: *mut sender_t) {
        let _ = Box::from_raw(sender); // Drop the value
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Other(e)
    }
}

impl From<serialport::Error> for Error {
    fn from(e: serialport::Error) -> Self {
        Self::Open(e)
    }
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::WrongDevice => write!(f, "the device connected is not responding correctly. Try resetting it and check the loaded code."),
            Self::Disconnected => write!(f, "device disconnected unexpectedly"),
            Self::UnsupportedVersion(v) => write!(f, "device implements an unsupported protocol version: {}", v),
            Self::InvalidSpeed => write!(f, "the device could not set the speed"),
            Self::Open(e) => write!(f, "could not open serial port file: {}", e),
            Self::Other(e) => write!(f, "error during transmission: {}", e),
        }
    }
}

pub fn communicate<P: AsRef<Path>>(
    serial: P,
) -> Result<(mpsc::Sender<i16>, JoinHandle<Result<(), Error>>), Error> {
    let settings = SerialPortSettings {
        baud_rate: 115200,
        timeout: Duration::from_millis(1000),
        ..SerialPortSettings::default()
    };
    let mut serial = serialport::open_with_settings(serial.as_ref(), &settings)?;
    thread::sleep(Duration::from_millis(2000));
    serial.write_all(b"smov")?;
    let mut buf = [0; 4];
    serial.read_exact(&mut buf)?;
    if &buf != b"smov" {
        return Err(Error::WrongDevice);
    }
    let mut version = [0; 2];
    serial.read_exact(&mut version)?;
    let version = u16::from_be_bytes(version);
    if version == 0 {
        serial.write_all(&[0x00])?;
    } else {
        serial.write_all(&[0x01])?;
        return Err(Error::UnsupportedVersion(version));
    }
    let (tx, rx) = mpsc::channel::<i16>();
    let handle = thread::spawn(move || loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(speed) => {
                let speed = speed.to_be_bytes();
                serial.write_all(&[0x01, speed[0], speed[1]])?;
                let mut result = [0; 1];
                serial.read_exact(&mut result)?;
                if result[0] != 0 {
                    return Err(Error::InvalidSpeed);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                serial.write_all(&[0x00])?;
                let mut result = [0; 1];
                serial.read_exact(&mut result)?;
                if result[0] != 0 {
                    return Err(Error::Disconnected);
                }
            }
            Err(_) => return Ok(()),
        }
    });
    Ok((tx, handle))
}
