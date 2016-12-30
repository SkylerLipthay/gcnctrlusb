# gcnctrlusb

Provides an interface for reading input from a *Nintendo GameCube Controller Adapter for Wii U* USB device.

Third party clones such as the 4-port Mayflash adapter in "Wii U mode" are also supported.

This library depends on `libusb`, which is available as a dynamic library on many platforms including Linux, Windows, and Mac OS X.

Currently, rumble commands are **unimplemented**.

## Usage

```rust
extern crate gcnctrlusb;

fn main() {
    // Panics if `libusb` is not found or otherwise fails.
    let mut scanner = gcnctrlusb::Scanner::new().unwrap();
    // Panics if a valid device was not found.
    let mut adapter = scanner.find_adapter().unwrap().unwrap();
    // Panics if the USB driver fails to open a connection to the device.
    let mut listener = adapter.listen().unwrap();

    while let Ok(controllers) = listener.read() {
        println!("Controller port 1: {:?}", controllers[0]);
    }
}
```

Try `cargo run --example log` for a pretty readout! This example requires a terminal that supports ANSI 256 colors and a terminal font that includes the bottom-half block character (▄). This example works best in a terminal with a black background.

![](www/example.gif?raw=true)
