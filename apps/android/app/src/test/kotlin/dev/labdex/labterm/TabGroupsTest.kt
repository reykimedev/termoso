package dev.labdex.labterm

import dev.labdex.labterm.data.CommandTarget
import dev.labdex.labterm.data.SessionHostInfo
import dev.labdex.labterm.data.Targetable
import dev.labdex.labterm.data.hostGroupPaths
import dev.labdex.labterm.data.nextTabGroupColor
import dev.labdex.labterm.data.resolveTargets
import dev.labdex.labterm.data.tagCovers
import dev.labdex.labterm.data.tagGroupNames
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TabGroupsTest {
    private data class S(
        override val id: String,
        override val groupIds: Set<String> = emptySet(),
        override val hostInfo: SessionHostInfo = SessionHostInfo(),
        val connected: Boolean = true,
    ) : Targetable

    private val sessions = listOf(
        S("a", setOf("web", "prod"), SessionHostInfo(listOf("kube.oak"), listOf("prod", "web"))),
        S("b", setOf("web"), connected = false),
        S("c", setOf("prod"), SessionHostInfo(listOf("kube"), listOf("prod"))),
        S("d", emptySet(), SessionHostInfo(listOf("kubernetes"), listOf("lab"))),
    )

    private fun ids(t: CommandTarget, active: String? = "a") =
        resolveTargets(sessions, active, t) { it.connected }.map { it.id }

    @Test fun activeAndAll() {
        assertEquals(listOf("a"), ids(CommandTarget.Active))
        assertEquals(listOf("a", "c", "d"), ids(CommandTarget.All))
        assertEquals(emptyList<String>(), ids(CommandTarget.Active, active = null))
    }

    @Test fun groupsOverlapButEachSessionOnce() {
        assertEquals(listOf("a"), ids(CommandTarget.Group("web")))
        assertEquals(listOf("a", "c"), ids(CommandTarget.Group("prod")))
    }

    @Test fun tagsAndHostGroupsCoverChildren() {
        assertEquals(listOf("a", "c"), ids(CommandTarget.Tag("kube")))
        assertEquals(listOf("a", "c"), ids(CommandTarget.HostGroup(listOf("prod"))))
        assertEquals(listOf("a"), ids(CommandTarget.HostGroup(listOf("prod", "web"))))
    }

    @Test fun nestedNames() {
        assertEquals(listOf("kube", "kube.oak", "prod"), tagGroupNames(listOf("kube.oak", "prod")))
        assertTrue(tagCovers("Kube", "kube.oak"))
        assertFalse(tagCovers("kube", "kubernetes"))
        assertEquals(
            listOf(listOf("lab"), listOf("prod"), listOf("prod", "web")),
            hostGroupPaths(listOf(listOf("prod", "web"), listOf("lab"))),
        )
    }

    @Test fun coloursRotate() {
        assertEquals(0xFF4F8FF7, nextTabGroupColor(emptyList()))
        assertEquals(0xFF3FB68B, nextTabGroupColor(listOf(0xFF4F8FF7)))
    }
}
