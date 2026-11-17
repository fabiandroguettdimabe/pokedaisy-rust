package com.pokedaisy.app.companion.data

import org.json.JSONObject

/** Versioned cross-platform envelope used by shared Rust/Kotlin contract fixtures. */
data class CompanionSessionSnapshot(
    val protocolVersion: Int,
    val sessionId: String,
    val epoch: Long,
    val revision: Long,
    val payload: JSONObject,
) {
    companion object {
        const val SUPPORTED_PROTOCOL_VERSION = 1

        fun parse(json: String): CompanionSessionSnapshot {
            val root = JSONObject(json)
            val version = root.requiredLong("protocol_version")
            require(version == SUPPORTED_PROTOCOL_VERSION.toLong()) {
                "Unsupported session snapshot protocol version: $version"
            }
            val sessionId = root.opt("session_id") as? String
                ?: throw IllegalArgumentException("session_id must be a string")
            require(sessionId.isNotBlank()) { "session_id must not be blank" }
            val epoch = root.requiredLong("epoch").also { require(it >= 0) { "epoch must be non-negative" } }
            val revision = root.requiredLong("revision").also { require(it >= 0) { "revision must be non-negative" } }
            val payload = root.opt("payload") as? JSONObject
                ?: throw IllegalArgumentException("payload must be an object")
            return CompanionSessionSnapshot(version.toInt(), sessionId, epoch, revision, payload)
        }

        private fun JSONObject.requiredLong(key: String): Long {
            val value = opt(key) as? Number ?: throw IllegalArgumentException("$key must be an integer")
            val longValue = value.toLong()
            require(value.toDouble() == longValue.toDouble()) { "$key must be an integer" }
            return longValue
        }
    }
}
