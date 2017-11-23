Usage
=====

Programming animations for Ledcat is simple! The interface has been designed to
be generic, language agnostic, easy to understand and allow reuse of programs
across projects/displays.

All you have to do is create a program that, for each pixel in your display,
outputs three bytes for each sub-pixel in RGB order. Ledcat knows how to
distinguish between frames because it knows how many byte a frame contains.


## Input
The simplest way of offering animation data to Ledcat is through it's STDIN:
```sh
perl -e 'print "\xff\x00\x00" x 30' | ledcat --geometry 30 <other arguments...>
```

### FIFO's
It is also possible to offer data to Ledcat by using one or more FIFO's. The
`--linger` is best used as well, since it tells Ledcat to retry reading when
you restart the animating program.
```sh
mkfifo /tmp/ledcat-01 /tmp/ledcat-02 /tmp/ledcat-03
ledcat --input /tmp/ledcat-01 /tmp/ledcat-02 /tmp/ledcat-03 --linger <other arguments...>
```
With this setup, Ledcat will prefer frames from the rightmost FIFO which can be
read from.


## Display Geometry
Besides the `--geometry` option, it is also possible to set the display
geometry via the `LEDCAT_GEOMETRY` environment variable. This allows programs
to be reused between displays with differing geometry without having to specify
the geometry twice. Both options expect an integer for 1D geometry and two
integers separated by an `x` for 2D.

### Transpositions
It is possible to modify which pixel goes where in the output. Accidentally
mounted your display upside down? No problem. Head over to the [transposition
doc](transposition.md) for more details.


## Timing
By default, Ledcat will just read frames from it's input and output them
immediately. To prevent hogging system resources with a busy loop, you should
include a sleep in your animations to sync up with the desired frame rate.

This approach is recommended for animations which should visualize another data
source in real time, like an audio signal.

If there are no real time requirements, it is recommended to leave out sleeps
in your program and set the desired frame rate with `--framerate`. Ledcat read
from it's input when needed and cause the animation program to block.

### The Clear Timeout
When you're using Ledcat like this (or with a network socket), it is a valid
use case to terminate the animating program to start a new one. It is possible
that a part of some frame that was produced by the previous animation is still
stuck in Ledcat's input buffers.

But fear not! Ledcat will automatically clear these leftovers if you wait for a
while. If we would not, we would end up with shifted frames and colors in the
output. The timeout is based on the frame rate set with `--framerate`,
`--clear-timeout` or a default of 100ms. You should wait this amount before
writing new animations.
