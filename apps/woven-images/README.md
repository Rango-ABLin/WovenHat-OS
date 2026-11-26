# WovenImages

WovenImages is a bounded Ring-3 image inventory and viewer. It lists the first
128 entries under a selected directory, recognizes PNG/JPEG/GIF/BMP/WebP
signatures, and decodes bounded BMP files selected by path in the GUI into a
fixed 160x90 display budget.

Modes are `--list PATH`, `--info PATH`, and `--help`. The GUI has a path field
and Load action; only BMP decoding is compiled in because adding the PNG/JPEG/
GIF/WebP decoders pushed the signed WAPP past the bounded VFS node cap and
broke the system-app catalog install. That limit is recorded, not hidden by
widening the install boundary. A pre-existing startup check that compared the
32x32 thumbnail to the 160x90 source geometry was also removed so the window
can actually open.
