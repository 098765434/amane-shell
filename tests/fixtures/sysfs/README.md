# sysfs fixtures

`/sys/class/rfkill` and `/sys/class/leds` from a ThinkPad, copied as plain
files (sysfs entries are links there). Changed from the recording: caps lock
is lit, and the keyboard light is at level 1 of 2, so the tests read a
non-zero value.
