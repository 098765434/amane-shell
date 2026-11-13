# Network

Emaki Settings' **Network** page (`apps/settings/src/network/`) covers:
- Wi‑Fi, Ethernet, VPNs and the hotspot;
- a page for each connection with its addresses and IPv4 settings.

`emaki-settings --page network` opens it, and so do `wifi`, `vpn`, `ethernet` and `hotspot`. Quick
Settings' Wi‑Fi and VPN pages and the Network widget open it as well. Without Emaki Settings they
open `nm-connection-editor`.

Joining a network in range is in Quick Settings' Wi‑Fi page and in the launcher's **Wi‑Fi
Networks** ([launcher.md](launcher.md#wi-fi-and-bluetooth)).

## The page

- **Wi‑Fi:**
  - The radio's switch, which can't be turned on while a hardware switch holds it off.
  - The network joined. Its row opens its connection page.
  - **Saved networks**: every Wi‑Fi profile, the last used first. Each one opens its connection page.
  - **Join a hidden network**: its name and password (empty for an open network). It shows
    Connecting…, Connected, or why it failed.
- **Ethernet:** shown while a cable is in. It has a switch to connect or disconnect, and Details
  while it's up.
- **VPN:**
  - Each WireGuard or VPN profile, with a switch and a button to its page.
  - **Import from File…** takes a WireGuard `.conf` or an OpenVPN `.ovpn` file.
- **Hotspot:**
  - Its name, which starts as the machine's name.
  - Its password, with a lock button to show it. An empty password gets one made, and the page then
    shows it.
  - **Share this connection** switch. While it runs, this machine's Wi‑Fi can't join other networks.
- **More options** opens `nm-connection-editor` for what the page doesn't set: 802.1X, proxies,
  IPv6, and a VPN's own settings.

### A connection's page

- **Kind and state**, and **Connect automatically**.
- **Details** while it's up:
  - the IPv4 address, gateway and DNS servers;
  - the IPv6 address;
  - the interface and hardware address;
  - the link speed (Ethernet) or band and frequency (Wi‑Fi).

  They're read again every 3 s while the page shows.
- **IPv4:**
  - Method: Automatic (DHCP), Manual, Link-Local Only or Disabled.
  - Manual adds Address, Prefix and Gateway.
  - Automatic has an **Automatic DNS** switch. With it on, the servers typed are used as well as
    the network's. With it off, only the servers typed are used.
  - **Apply** checks the form first. For example, an address needs four numbers from 0 to 255,
    and a prefix is 1 to 32.
  - A connection that's up asks before Apply reconnects it with the new settings. **Revert** puts
    back what's saved.
- **Forget This Network…** (Wi‑Fi) or **Remove This VPN…** deletes the profile with its password
  after a confirmation.

Searching "dns" or "ip address" with nothing chosen opens the page of the connection carrying the
traffic.

## Where the changes go

Everything is NetworkManager's own profiles, so a change shows wherever else it's seen: Quick
Settings, `nmcli`, `nm-connection-editor`.

- **Through amane's `Network`, over D-Bus** (`../amane/docs/fork/system/network.md`):
  - reading the profiles and a connection's details;
  - joining, including a hidden network;
  - bringing a profile up or down;
  - forgetting one.
- **Through `nmcli`** (`network/nmcli.rs`):
  - changing a saved profile (IPv4, DNS, autoconnect);
  - importing a VPN;
  - the hotspot.

  `nmcli connection modify` keeps the profile's stored secrets. A D-Bus `Update` replaces the whole
  profile, so it would need them read back first. nmcli also carries the VPN plugins' importers.
  A job shows Working… and then what happened at the top of the page. A failure shows nmcli's own
  words.

**Profiles set in NixOS.** A profile written by NixOS's `networking.networkmanager.ensureProfiles`
lives under `/run/NetworkManager/system-connections`, so the next boot writes it again.
- Such a profile says "Set in NixOS".
- It can't be forgotten or changed here, and its page says to change it in the configuration.
- amane calls it `declared`, from the profile's file being under `/run/`.

**Importing VPNs.**
- WireGuard is built into NetworkManager.
- OpenVPN needs NetworkManager's plugin, `networking.networkmanager.plugins = [
  pkgs.networkmanager-openvpn ];`. Without it the VPN group says so, and an `.ovpn` file isn't
  imported.
- The page finds the plugin by its `nm-openvpn-service.name` under `/etc/NetworkManager/VPN` or a
  `lib/NetworkManager/VPN` folder.

**The hotspot** is `nmcli device wifi hotspot con-name Hotspot ssid <name> [password <password>]`.
It makes the `Hotspot` profile, or changes it, so the name and password are kept for next time.
Turning it off takes that profile down.

## Not yet

- Editing IPv6, and per-network MAC randomisation.
- 802.1X and enterprise Wi‑Fi.
- Proxies.
- A VPN's own settings beyond importing it (in `nm-connection-editor` for now).
- Mobile broadband.
