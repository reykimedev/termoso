import { navStore, goToSection } from "@/app/navigation";
import type { Uuid } from "@/ipc/types";

/** Open the VPN networks page on one network's details (host badges, the
 *  network picker). */
export function openVpnNetwork(id: Uuid) {
  navStore.set((s) => ({ ...s, pendingVpnNetwork: id }));
  goToSection("vpnNetworks");
}
