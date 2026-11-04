# §562. A Panasonic v4 window read within its block

*2026-11-04*

A tester's Lumix GX9 frames showed short blue dashes that Lightroom
and darktable did not (issue 66). The frame is RawFormat 4, 12-bit,
in split 16 KB blocks, and the dashes were in the decode: rawler's
Panasonic v4 bit pump maps a 16-bit window's first byte through the
block's 0x2008 split and then takes the second byte from the next
byte of the file. Once a block, a read straddles the split, and its
high nibble comes from the next block instead of the block's own
first byte. On the frame that was one group of 14 pixels wrong in
every block, 1041 of them: most off by multiples of 16 and unseen,
24 pixels off by over 1024, which were the dashes. Every split v4
RW2 is affected to some degree.

We found it by decoding the frame with LibRaw and with rawler and
comparing: 4152 pixels differed, every one at group positions 6, 8,
10 and 12, every one in a block's first group. A Python port of
LibRaw's `panasonic_load_raw` matched LibRaw exactly and, traced,
put every wrong read at the same place: bit 4 of byte 0x2007, the
window across the split.

The fix maps each of the window's two bytes on its own and reads zero
past the block's end, as LibRaw's buffer does. The decode then
matches LibRaw bit for bit. It is dnglab/dnglab#925, and the fork's
pinned branch carries it until a rawler release does.
