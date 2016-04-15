//! Provides an interface for reading input from a *Nintendo GameCube Controller Adapter for Wii U*
//! USB device.
//!
//! Third party clones such as the 4-port Mayflash adapter in "PC mode" are also supported.
//!
//! This library depends on `libusb`, which is available as a dynamic library on many platforms
//! including Linux, Windows, and Mac OS X.
//!
//! Currently, rumble commands are **unimplemented**.
//!
//! # Usage
//!
//! ```norun
//! extern crate gcnctrlusb;
//!
//! fn main() {
//!     // Panics if `libusb` is not found or otherwise fails.
//!     let mut scanner = gcnctrlusb::Scanner::new().unwrap();
//!     // Panics if a valid device was not found.
//!     let mut adapter = scanner.find_adapter().unwrap().unwrap();
//!     // Panics if the USB driver fails to open a connection to the device.
//!     let mut listener = adapter.listen().unwrap();
//!
//!     while let Ok(controllers) = listener.read() {
//!         println!("Controller port 1: {:?}", controllers[0]);
//!     }
//! }
//! ```

extern crate libusb;

use libusb::{Context, Device, DeviceHandle};
use std::error::Error as StdError;
use std::fmt::Error as FmtError;
use std::fmt::{Display, Formatter};
use std::time::Duration;

const VENDOR_ID: u16 = 0x057e;
const PRODUCT_ID: u16 = 0x0337;

/// Searches for GameCube controller adapter USB devices.
pub struct Scanner {
    context: Context,
}

impl Scanner {
    /// Initializes USB driver connectivity and returns a `Scanner` instance.
    ///
    /// An error is returned if `libusb` is not loaded or driver initialization otherwise fails.
    pub fn new() -> Result<Scanner, Error> {
        Ok(Scanner { context: try!(Context::new()) })
    }

    /// Returns the first adapter found, or `None` if no adapter was found.
    pub fn find_adapter<'a>(&'a mut self) -> Result<Option<Adapter<'a>>, Error> {
        for mut device in try!(self.context.devices()).iter() {
            let desc = try!(device.device_descriptor());

            if desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID {
                return Ok(Some(Adapter { device: device }));
            }
        }

        Ok(None)
    }
}

/// A wrapper around the unopened USB device.
pub struct Adapter<'a> {
    device: Device<'a>,
}

impl<'a> Adapter<'a> {
    /// Opens the USB device and initializes the hardware for reading controller data.
    ///
    /// If the device is inaccessible or unrecognizable, an error is returned. For example, the
    /// device will be inaccessible if a previous `Listener` for this adapter is still alive.
    pub fn listen(&mut self) -> Result<Listener<'a>, Error> {
        let mut handle = try!(self.device.open());

        let config = try!(self.device.config_descriptor(0));

        let mut interface_descriptor: Option<_> = None;
        let mut endpoint_in = None;
        let mut endpoint_out = None;

        for interface in config.interfaces() {
            interface_descriptor = None;
            endpoint_in = None;
            endpoint_out = None;
            for desc in interface.descriptors() {
                for endpoint in desc.endpoint_descriptors() {
                    match endpoint.direction() {
                        libusb::Direction::In => endpoint_in = Some(endpoint.address()),
                        libusb::Direction::Out => endpoint_out = Some(endpoint.address()),
                    }
                }
                interface_descriptor = Some(desc);
            }
        }

        if interface_descriptor.is_none() || endpoint_in.is_none() || endpoint_out.is_none() {
            return Err(Error::UnrecognizedProtocol);
        }

        let interface_descriptor = interface_descriptor.unwrap();
        let interface_number = interface_descriptor.interface_number();

        let has_kernel_driver = match handle.kernel_driver_active(interface_number) {
            Ok(true) => {
                try!(handle.detach_kernel_driver(interface_number));
                true
            },
            _ => false,
        };

        try!(handle.set_active_configuration(config.number()));
        try!(handle.claim_interface(interface_number));
        let setting = interface_descriptor.setting_number();
        try!(handle.set_alternate_setting(interface_number, setting));

        // Tell the adapter to start sending packets.
        let timeout = Duration::from_secs(1);
        try!(handle.write_interrupt(endpoint_out.unwrap(), &[0x13], timeout));

        Ok(Listener {
            handle: handle,
            buffer: [0; 37],
            has_kernel_driver: has_kernel_driver,
            interface: interface_number,
            endpoint_in: endpoint_in.unwrap(),
        })
    }
}

/// An interface that reads packets of controller data on each iteration.
///
/// This interface owns an opened handle to the USB device that is closed once the `Listener`
/// instance is dropped.
pub struct Listener<'a> {
    handle: DeviceHandle<'a>,
    buffer: [u8; 37],
    has_kernel_driver: bool,
    interface: u8,
    endpoint_in: u8,
}

impl<'a> Listener<'a> {
    /// Reads a data packet and returns the states for each of the four possibly connected
    /// controllers.
    ///
    /// If reading a single packet takes over 1 second, a timeout error with occur. In testing,
    /// these packets are available at over 100 times per second.
    ///
    /// Reasons an error may occur include:
    ///
    /// * The USB device becomes disconnected
    /// * The USB driver throws an error, fatal or not
    /// * A USB message was successfully read, but it was not the right size
    ///
    /// It is wise to treat all errors returned as fatal, and to reestablish the adapter connection
    /// through `Scanner::find_adapter`.
    pub fn read(&mut self) -> Result<[Option<Controller>; 4], Error> {
        let timeout = Duration::from_secs(1);
        match self.handle.read_interrupt(self.endpoint_in, &mut self.buffer, timeout) {
            Ok(read) if read == 37 => Ok(Controller::parse_packet(&self.buffer)),
            Ok(_) => Err(Error::InvalidPacket),
            Err(err) => Err(Error::Usb(err)),
        }
    }
}

impl<'a> Drop for Listener<'a> {
    fn drop(&mut self) {
        if self.has_kernel_driver {
            let _ = self.handle.attach_kernel_driver(self.interface);
        }
    }
}

/// The state of a GameCube controller at a given moment in time.
///
/// Note that the hardware will likely never report either extreme of the spectrum for any of the
/// analog inputs. For example, all `u8` fields may report only within the range of `30` to `225`.
/// Also, the hardware will likely never report a perfect `127` for the resting position of any of
/// the joystick axes. Keep in my that this library does not do any analog dead zone correction.
#[derive(Clone, Copy, Debug)]
pub struct Controller {
    /// The classification of this controller.
    pub kind: ControllerKind,
    /// "A" button status.
    pub a: bool,
    /// "B" button status.
    pub b: bool,
    /// "X" button status.
    pub x: bool,
    /// "Y" button status.
    pub y: bool,
    /// Directional pad up button status.
    pub up: bool,
    /// Directional pad down button status.
    pub down: bool,
    /// Directional pad left button status.
    pub left: bool,
    /// Directional pad right button status.
    pub right: bool,
    /// Digital "L" button (full depression) status.
    pub l: bool,
    /// Digital "R" button (full depression) status.
    pub r: bool,
    /// The level of depression of the analog "L" button, `0` being completely up, `255` being
    /// completely pressed in.
    pub l_analog: u8,
    /// The level of depression of the analog "R" button, `0` being completely up, `255` being
    /// completely pressed in.
    pub r_analog: u8,
    /// "Z" button status.
    pub z: bool,
    /// Start button status.
    pub start: bool,
    /// The x-axis position of the primary analog joystick, `0` being completely left, `255` being
    /// completely right.
    pub stick_x: u8,
    /// The y-axis position of the primary analog joystick, `0` being completely down, `255` being
    /// completely up.
    pub stick_y: u8,
    /// The x-axis position of the secondary ("C") analog joystick, `0` being completely left,
    /// `255` being completely right.
    pub c_stick_x: u8,
    /// The y-axis position of the secondary ("C") analog joystick, `0` being completely down,
    /// `255` being completely up.
    pub c_stick_y: u8,
}

impl Controller {
    // # Panics
    //
    // Panics if `data` is not at least 9 bytes.
    fn parse(data: &[u8]) -> Option<Controller> {
        let kind = match data[0] >> 4 {
            0 => return None,
            1 => ControllerKind::Wired,
            2 => ControllerKind::Wireless,
            _ => ControllerKind::Unknown,
        };

        Some(Controller {
            kind: kind,
            a: data[1] & (1 << 0) != 0,
            b: data[1] & (1 << 1) != 0,
            x: data[1] & (1 << 2) != 0,
            y: data[1] & (1 << 3) != 0,
            left: data[1] & (1 << 4) != 0,
            right: data[1] & (1 << 5) != 0,
            down: data[1] & (1 << 6) != 0,
            up: data[1] & (1 << 7) != 0,
            start: data[2] & (1 << 0) != 0,
            z: data[2] & (1 << 1) != 0,
            r: data[2] & (1 << 2) != 0,
            l: data[2] & (1 << 3) != 0,
            stick_x: data[3],
            stick_y: data[4],
            c_stick_x: data[5],
            c_stick_y: data[6],
            l_analog: data[7],
            r_analog: data[8],
        })
    }

    // # Panics
    //
    // Panics if `data` is not at least 37 bytes.
    fn parse_packet(data: &[u8]) -> [Option<Controller>; 4] {
        [
            Controller::parse(&data[1..10]),
            Controller::parse(&data[10..19]),
            Controller::parse(&data[19..28]),
            Controller::parse(&data[28..37])
        ]
    }
}

/// The classification of a GameCube controller.
#[derive(Clone, Copy, Debug)]
pub enum ControllerKind {
    /// The controller is wired and likely supports rumble.
    Wired,
    /// The controller is wireless and likely does not supports rumble.
    Wireless,
    /// The controller is of an unknown type.
    Unknown,
}

/// An error that occurs during usage of this library.
#[derive(Debug)]
pub enum Error {
    /// A USB driver error that can occur at any time while utilizing this library.
    Usb(libusb::Error),
    /// A seemingly valid adapter was found, but its communication protocol could not be resolved.
    UnrecognizedProtocol,
    /// An invalid message was read from the adapter, likely due to a device or driver failure.
    InvalidPacket,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Usb(ref err) => err.description(),
            Error::UnrecognizedProtocol => "USB adapter protocol unrecognized",
            Error::InvalidPacket => "Invalid data packet received",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::Usb(ref err) => err.cause(),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            Error::Usb(ref err) => Display::fmt(err, f),
            _ => self.description().fmt(f),
        }
    }
}

impl From<libusb::Error> for Error {
    fn from(err: libusb::Error) -> Error {
        Error::Usb(err)
    }
}
