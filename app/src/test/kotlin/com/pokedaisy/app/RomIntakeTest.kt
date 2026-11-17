package com.pokedaisy.app

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Test

class RomIntakeTest {
    @Test fun copyCappedAcceptsExactLimit() {
        val bytes = byteArrayOf(1, 2, 3, 4)
        val output = ByteArrayOutputStream()

        RomIntake.copyCapped(ByteArrayInputStream(bytes), output, maxBytes = bytes.size.toLong())

        assertArrayEquals(bytes, output.toByteArray())
    }

    @Test fun copyCappedRejectsOverflowBeforeWritingTheExcessChunk() {
        val output = ByteArrayOutputStream()

        try {
            RomIntake.copyCapped(ByteArrayInputStream(byteArrayOf(1, 2, 3, 4)), output, maxBytes = 3)
            fail("expected an oversized ROM import to be rejected")
        } catch (expected: IOException) {
            assertEquals("more than 3 bytes - not a ROM", expected.message)
        }

        assertEquals(0, output.size())
    }
}
