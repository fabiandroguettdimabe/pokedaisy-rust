"""Build an original, logo-free NDS test cart: ARM9 draws input feedback; ARM7 loops.

No game, firmware, BIOS or Nintendo header logo is copied. ARM instructions
are emitted directly so this diagnostic does not need a devkitARM install.
"""
from pathlib import Path
import struct
import sys


def diagnostic():
    words, literals = [], []

    def load(register, value):
        literals.append((len(words), register, value))
        words.append(0)

    load(0, 0x04000304)
    load(1, 0x8203)
    words.append(0xe1c010b0)  # strh r1, [r0]: power LCD + main engine
    load(0, 0x04000000)
    load(1, 0x00020000)
    words.append(0xe5801000)  # str r1, [r0]: VRAM display mode
    load(0, 0x04000240)
    words += [0xe3a01080, 0xe5c01000]  # mov r1,#0x80; strb: VRAM A LCDC
    load(0, 0x06800000)
    load(1, 0x83e0)  # opaque green RGB555
    load(2, 256 * 192)
    words += [0xe0c010b2, 0xe2522001, 0x1afffffc]
    # Four corner markers prove the host displays every source pixel, not a crop.
    for pixel, color in [(0,0x801f),(255,0xfc00),(191*256,0xffff),(192*256-1,0x83ff)]:
        load(0,0x06800000+pixel*2);load(1,color);words.append(0xe1c010b0)
    # Poll KEYINPUT (active-low A) and repaint the framebuffer red while held,
    # green when released. This makes real frontend input observable in SDL.
    load(4,0x04000130)
    poll_start=len(words)
    words += [
        0xe1d410b0, # ldrh r1,[r4]: KEYINPUT
        0xe3110001, # tst r1,#1: A is pressed when bit 0 is clear
        0x03a02080, # moveq r2,#0x80
        0x13a02083, # movne r2,#0x83
        0xe1a02402, # mov r2,r2,lsl #8
        0x0382201f, # orreq r2,r2,#0x1f (opaque red)
        0x138220e0, # orrne r2,r2,#0xe0 (opaque green)
    ]
    load(0,0x06800000)
    load(1,256*192)
    pixel_loop=len(words)
    words += [0xe0c020b2,0xe2511001]
    branch=len(words);words.append(0)
    words[branch]=0x1a000000|((pixel_loop-branch-2)&0xffffff)
    branch=len(words);words.append(0)
    words[branch]=0xea000000|((poll_start-branch-2)&0xffffff)
    for index, register, value in literals:
        distance = (len(words) - index) * 4 - 8
        words[index] = 0xe59f0000 | register << 12 | distance
        words.append(value)
    arm9 = struct.pack('<' + 'I' * len(words), *words)
    rom = bytearray(0x10000)
    rom[:12] = b'PD DS TEST  '
    rom[12:16] = b'DSPT'
    struct.pack_into('<8I', rom, 0x20, 0x200, 0x02000000, 0x02000000,
                     len(arm9), 0x400, 0x03800000, 0x03800000, 4)
    struct.pack_into('<II', rom, 0x80, len(rom), 0x200)
    rom[0x200:0x200 + len(arm9)] = arm9
    struct.pack_into('<I', rom, 0x400, 0xeafffffe)
    return rom


if __name__ == '__main__':
    Path(sys.argv[1]).write_bytes(diagnostic())
