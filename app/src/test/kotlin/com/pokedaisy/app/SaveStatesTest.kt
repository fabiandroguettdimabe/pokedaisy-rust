package com.pokedaisy.app

import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Legacy save/state folders are keyed by CRC32 of ROM bytes; keep this stable across app upgrades. */
class SaveStatesTest {
    @get:Rule val tmp = TemporaryFolder()

    @Test fun crc32KnownVectorKeepsLegacyDirectoryKey() {
        val rom = tmp.newFile("renamed-rom.gba").apply { writeText("123456789") }
        assertEquals("cbf43926", SaveStates.crc32(rom))
    }

    @Test fun crc32DoesNotDependOnRomFileName() {
        val first = tmp.newFile("first.gba").apply { writeBytes(byteArrayOf(1, 2, 3, 4)) }
        val renamed = tmp.newFile("renamed.gba").apply { writeBytes(byteArrayOf(1, 2, 3, 4)) }
        assertEquals(SaveStates.crc32(first), SaveStates.crc32(renamed))
    }
}
