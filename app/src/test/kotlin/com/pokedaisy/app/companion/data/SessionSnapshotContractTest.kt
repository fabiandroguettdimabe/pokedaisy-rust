package com.pokedaisy.app.companion.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.io.File

class SessionSnapshotContractTest {
    private val root = generateSequence(File("").absoluteFile) { it.parentFile }
        .first { File(it, "settings.gradle.kts").isFile }

    @Test
    fun sharedV1FixtureDecodesWithTheRustFieldNames() {
        val fixture = File(root, "contracts/vyntra/v1/session-snapshot-v1.json").readText()
        val snapshot = CompanionSessionSnapshot.parse(fixture)
        assertEquals(1, snapshot.protocolVersion)
        assertEquals("fixture-session", snapshot.sessionId)
        assertEquals(3L, snapshot.epoch)
        assertEquals(8L, snapshot.revision)
        assertEquals("BPRE", snapshot.payload.getString("game_code"))
    }

    @Test
    fun incompatibleMajorFixtureIsRejected() {
        val fixture = File(root, "contracts/vyntra/v1/session-snapshot-v2-incompatible.json").readText()
        assertThrows(IllegalArgumentException::class.java) {
            CompanionSessionSnapshot.parse(fixture)
        }
    }

    @Test
    fun nonObjectPayloadIsRejected() {
        val fixture = File(root, "contracts/vyntra/v1/session-snapshot-v1.json")
            .readText().replace("\"payload\": { \"game_code\": \"BPRE\", \"paused\": false }", "\"payload\": []")
        assertThrows(IllegalArgumentException::class.java) {
            CompanionSessionSnapshot.parse(fixture)
        }
    }
}
