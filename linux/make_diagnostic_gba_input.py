"""Build a tiny original GBA ROM whose blue/green screen proves A-button input."""
import struct
import sys
from pathlib import Path


rom = bytearray(32768)
rom[0:4] = struct.pack("<I", 0xEA00002E)  # Branch to 0xC0.
rom[0xA0:0xAC] = b"PD INPUT GBA"
rom[0xAC:0xB0] = b"PDIN"
rom[0xB2] = 0x96
rom[0xBC] = 1

words = []
labels = {}
branches = []
literals = []


def emit(word):
    words.append(word)


def label(name):
    labels[name] = len(words)


def ldr_literal(register, name):
    literals.append((len(words), register, name))
    emit(0xE59F0000 | (register << 12))


def branch(condition, name):
    branches.append((len(words), condition, name))
    emit(0)


# Enable mode 3, then paint each frame blue until GBA key A is held (active low).
ldr_literal(0, "io")
ldr_literal(1, "mode")
emit(0xE1C010B0)  # STRH r1, [r0]
ldr_literal(0, "keyinput")
ldr_literal(1, "vram")
ldr_literal(2, "pixels")
label("loop")
emit(0xE1D030B0)  # LDRH r3, [r0]
emit(0xE3130001)  # TST r3, #1
branch(0x0, "pressed")
ldr_literal(4, "blue")
branch(0xE, "draw")
label("pressed")
ldr_literal(4, "green")
label("draw")
emit(0xE1A05001)  # MOV r5, r1
emit(0xE1A06002)  # MOV r6, r2
label("fill")
emit(0xE0C540B2)  # STRH r4, [r5], #2
emit(0xE2566001)  # SUBS r6, r6, #1
branch(0x1, "fill")
branch(0xE, "loop")

literal_index = len(words)
literal_values = {
    "io": 0x04000000,
    "mode": 0x0403,
    "keyinput": 0x04000130,
    "vram": 0x06000000,
    "pixels": 240 * 160,
    "blue": 0x001F,
    "green": 0x03E0,
}
literal_positions = {name: literal_index + offset for offset, name in enumerate(literal_values)}
for instruction, register, name in literals:
    instruction_address = 0xC0 + instruction * 4
    literal_address = 0xC0 + literal_positions[name] * 4
    distance = literal_address - (instruction_address + 8)
    assert 0 <= distance < 4096
    words[instruction] |= distance
for instruction, condition, name in branches:
    delta = labels[name] - (instruction + 2)
    words[instruction] = (condition << 28) | 0x0A000000 | (delta & 0x00FFFFFF)
words.extend(literal_values.values())
rom[0xC0 : 0xC0 + len(words) * 4] = struct.pack("<" + "I" * len(words), *words)
rom[0xBD] = (-sum(rom[0xA0:0xBD]) - 0x19) & 0xFF
Path(sys.argv[1]).write_bytes(rom)
