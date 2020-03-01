ledcat
======
![CI](https://github.com/polyfloyd/ledcat/workflows/CI/badge.svg)
[![](https://img.shields.io/crates/v/ledcat.svg)](https://crates.io/crates/ledcat)

Ledcat is simple utility that aims to provide a standard interface for driving
LED-strips and such.

Simply create a program that outputs 3 bytes of RGB for each pixel in your strip.

## Documentation
* [Usage](doc/usage.md)
* [Transposition and display geometry](doc/transposition.md)

## Install
The easiest way to install Ledcat is to [download a binary from
Github](https://github.com/polyfloyd/ledcat/releases).

*Note: Hzeller's LED Matrix driver is not available from CI builds.*

### Installing from Cargo
Install the [Rust Language](https://www.rust-lang.org/) if you have not already.

Then, you can install ledcat directly using Cargo.
```sh
cargo install ledcat
```

### Building Manually
Alternatively, you can build and install Ledcat manually:
```sh
git clone https://github.com/polyfloyd/ledcat.git
cd ledcat
cargo build --release
cp target/release/ledcat /usr/local/bin/ledcat
```

## Usage Examples
```sh
# Make a strip of 30 apa102 leds all red.
perl -e 'print "\xff\x00\x00" x 30' | ledcat --geometry 30 apa102 > /dev/spidev0.0
```
```sh
# Receive frames over UDP.
nc -ul 1337 | ledcat --geometry 30 apa102 > /dev/spidev0.0
```
```sh
# Load an image named "image.png", resize it to fit the size of the display and
# send it to a ledstrip zigzagged over the Y-axis.
convert image.png -resize 75x8! -depth 8 RGB:- | \
    ledcat --geometry 75x8 --transpose zigzag_y apa102 > /dev/spidev0.0
```
```sh
# A clock on a zigzagged two dimensional display of 75x8 pixels
while true; do
    convert -background black -fill cyan -font Courier -pointsize 8 \
        -size 75x8 -gravity center -depth 8 caption:"$(date +%T)" RGB:-
    sleep 1;
done | ledcat --geometry 75x16 --transpose zigzag_y apa102 > /dev/spidev0.0;
```
```sh
# Show random noise as ambient lighting or priority messages if there are any.
mkfifo /tmp/ambient
mkfifo /tmp/messages
cat /dev/urandom > /tmp/ambient &
./my_messages > /tmp/messages &
ledcat --input /tmp/ambient /tmp/messages --exit never --geometry 30 apa102 > /dev/spidev0.0
```

### Supported Drivers:
* Linux [spidev](https://www.kernel.org/doc/Documentation/spi/spidev)
* Serial
* Artnet DMX

### Supported Device Types:
* show (emulates a LED bar in the terminal)
* apa102
* [HexWS2811](https://github.com/brainsmoke/hex2811-penta)
* lpd8806
* [hub75](doc/hub75.md)
* sk9822
* ws2812
* [LED Matrices on Raspberry Pi's](https://github.com/hzeller/rpi-rgb-led-matrix) (ARM+Linux only)
