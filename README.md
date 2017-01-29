ledcat
======

ledcat is simple utility that aims to be able to drive any kind of LED-strip
while also providing a simple standard interface for programs.

Simply create a program that outputs 3 bytes of RGB for each pixel in your strip.

## Usage Examples
```sh
# Make a strip of 30 apa102 leds all red.
perl -e 'print "\xff\x00\x00" x 30' | ledcat --num-pixels 30 apa102 > /dev/spidev0.0
```
```sh
# Receive frames over UDP.
nc -ul 1337 | ledcat --num-pixels 30 apa102 > /dev/spidev0.0
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
ledcat --input /tmp/ambient /tmp/messages --linger --num-pixels 30 apa102 > /dev/spidev0.0
```

### Supported Drivers:
* Linux [spidev](https://www.kernel.org/doc/Documentation/spi/spidev)
* Artnet DMX

### Supported Device Types:
* apa102
* lpd8806
