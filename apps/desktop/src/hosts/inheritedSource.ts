import type { Inherited } from "@/ipc/types";
import { tr } from "@/i18n";

/**
 * Where inherited settings come from, for "Inherited from …" hints: the
 * group path, then the tags that carry settings (`Prod / EU, tag “ubuntu”`).
 * `null` when nothing is inherited.
 */
export function inheritedSource(inh: Inherited | null | undefined): string | null {
  if (!inh) return null;
  const parts = [
    ...(inh.groupPath.length > 0 ? [inh.groupPath.join(" / ")] : []),
    ...inh.tags.map((t) => tr("tag “{label}”", { label: t })),
  ];
  return parts.length > 0 ? parts.join(", ") : null;
}
