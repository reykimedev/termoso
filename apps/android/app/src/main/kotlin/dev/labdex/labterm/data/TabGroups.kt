package dev.labdex.labterm.data

/** A named, coloured set of sessions the command window can send to. A session can be in several. */
data class TabGroup(val id: String, val name: String, val color: Long)

/** Where the command window sends a line. */
sealed interface CommandTarget {
    data object Active : CommandTarget
    data object All : CommandTarget
    data class Group(val id: String) : CommandTarget
    /** Every session whose host carries this tag or a sub-tag of it (`kube` covers `kube.oak`). */
    data class Tag(val tag: String) : CommandTarget
    /** Every session whose host is in this host group or below it (labels from the vault root). */
    data class HostGroup(val path: List<String>) : CommandTarget
}

/** What a session's saved host contributes to automatic targets. */
data class SessionHostInfo(val tags: List<String> = emptyList(), val groupPath: List<String> = emptyList())

/** Tab group colours (ARGB), handed out in turn; the same set the desktop uses. */
val TAB_GROUP_COLORS = listOf(
    0xFF4F8FF7, 0xFF3FB68B, 0xFFE0A030, 0xFFD9534F,
    0xFF9B6CF0, 0xFF2BB3C0, 0xFFE66FB0, 0xFF8A9A5B,
)

fun nextTabGroupColor(used: Collection<Long>): Long =
    TAB_GROUP_COLORS.firstOrNull { it !in used } ?: TAB_GROUP_COLORS[used.size % TAB_GROUP_COLORS.size]

/** `kube` covers `kube` and `kube.oak`, not `kubernetes`. Case-insensitive, like tag labels. */
fun tagCovers(group: String, tag: String): Boolean =
    tag.equals(group, ignoreCase = true) || tag.lowercase().startsWith(group.lowercase() + ".")

/** A host group path covers itself and everything below it. */
fun pathCovers(group: List<String>, path: List<String>): Boolean =
    group.isNotEmpty() && group.size <= path.size && group.indices.all { path[it] == group[it] }

/** Every tag group a set of host tags puts a session in: each tag and all its parents. */
fun tagGroupNames(tags: Collection<String>): List<String> =
    tags.flatMap { tag ->
        val parts = tag.split('.').filter { it.isNotEmpty() }
        (1..parts.size).map { parts.take(it).joinToString(".") }
    }.distinct().sortedWith(String.CASE_INSENSITIVE_ORDER)

/** Every host group the given host paths reach, parents included, in tree order. */
fun hostGroupPaths(paths: Collection<List<String>>): List<List<String>> =
    paths.flatMap { p -> (1..p.size).map { p.take(it) } }
        .distinct()
        .sortedWith { a, b -> a.joinToString("\u0000").compareTo(b.joinToString("\u0000")) }

/** The minimum a target needs to know about a session. */
interface Targetable {
    val id: String
    val groupIds: Set<String>
    val hostInfo: SessionHostInfo
}

/** Sessions a line sent to [target] reaches, each once, only those [writable] says can take input. */
fun <S : Targetable> resolveTargets(
    sessions: List<S>,
    activeId: String?,
    target: CommandTarget,
    writable: (S) -> Boolean,
): List<S> {
    val picked = when (target) {
        CommandTarget.Active -> sessions.filter { it.id == activeId }
        CommandTarget.All -> sessions
        is CommandTarget.Group -> sessions.filter { target.id in it.groupIds }
        is CommandTarget.Tag -> sessions.filter { s -> s.hostInfo.tags.any { tagCovers(target.tag, it) } }
        is CommandTarget.HostGroup -> sessions.filter { pathCovers(target.path, it.hostInfo.groupPath) }
    }
    return picked.distinctBy { it.id }.filter(writable)
}
