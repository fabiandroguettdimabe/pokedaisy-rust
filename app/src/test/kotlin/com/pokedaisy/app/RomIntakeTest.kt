package com.pokedaisy.app

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
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
        val input = object : InputStream() {
            private val bytes = byteArrayOf(1, 2, 3, 4)
            private var offset = 0

            override fun read(): Int = if (offset == bytes.size) -1 else bytes[offset++].toInt()

            override fun read(buffer: ByteArray, start: Int, length: Int): Int {
                if (offset == bytes.size) return -1
                val chunk = minOf(length, if (offset == 0) 3 else 1)
                bytes.copyInto(buffer, start, offset, offset + chunk)
                offset += chunk
                return chunk
            }
        }

        try {
            RomIntake.copyCapped(input, output, maxBytes = 3)
            fail("expected an oversized ROM import to be rejected")
        } catch (expected: IOException) {
            assertEquals("more than 3 bytes - not a ROM", expected.message)
        }

        assertArrayEquals(byteArrayOf(1, 2, 3), output.toByteArray())
    }
}
