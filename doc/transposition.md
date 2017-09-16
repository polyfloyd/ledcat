Transposition
=============

Ledcat shuffle pixels around before outputting them. This can be useful if the
arrangement of pixels in the output display does match that of the input. With
the `--transpose` option, you can set one or more operations to apply.

Because some operations are designed to work on 2 dimensional images, Some
operations need to know the dimensions of the display they are operating on.
Such operations require the number of pixels to be specified by the
`--geometry` option as WIDTHxHEIGHT.

## Reverse
![Reverse transposition](img/transpose-reverse.svg)

Applying `--transpose reverse` simply reverses the output image, meaning that
pixel 0 in the input becomes pixel N-1 in the output, 1 becomes N-2, etc.

Reversing a 2D image is equivalent to rotating by 180 degrees.

## Zig Zag
![Zigzagged X-axis transposition](img/transpose-zigzag_x.svg)

When building a display out of a single LED-strip, it is not uncommon to
arrange the strips in a repeating pattern that runs from left to right on the
first row and right to left on the next row.

Ledcat can transpose an input image with a regular left to right to match a
display with such zigzagged wiring. Setting Either `--transpose zigzag_x` or
`--transpose zigzag_y` will perform a zigzag transposition over the X- or
Y-axis respectively.

It is not possible to zigzag a 1D image, thus requiring the display size to be
configured using `--geomety` instead of `--num-pixels`.

## Mirror
Using `--transpose mirror_x` or `--transpose mirror_y` will mirror the output
image of the respective axis.
