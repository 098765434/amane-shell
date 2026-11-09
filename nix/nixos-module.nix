# The system half of emaki: the helper Emaki Settings' System pages apply a built
# configuration with, as root, through polkit. Import it from your configuration's flake
# (`emaki.nixosModules.default`) and set `programs.emaki.nixosActivate.enable = true;`.
#
# Settings builds the configuration as you and only hands the helper a store path; root never
# evaluates the repo. Anything that runs as you can call `pkexec emaki-nixos-activate` too,
# so the helper is strict about what it activates, every call asks for the password (no
# auth_admin_keep), and the prompt shows the store path it's for. Look at it before you
# type your password: a path you didn't just build in Settings is not yours.
#
# Replaces the helper `modules/managed.nix` used to define; remove that one when you enable
# this, or the two collide.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.emaki.nixosActivate;

  activate = pkgs.writeShellApplication {
    name = "emaki-nixos-activate";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.util-linux # logger
      config.nix.package
    ];
    text = ''
      # emaki-nixos-activate <switch|boot|test> </nix/store/<hash>-nixos-system-<name>>
      umask 077

      if [[ $# -ne 2 ]]; then
        echo "usage: emaki-nixos-activate <switch|boot|test> <store path>" >&2
        exit 2
      fi

      mode=$1
      out=$2

      [[ $mode =~ ^(switch|boot|test)$ ]] || { echo "bad mode" >&2; exit 2; }

      # exactly a NixOS system's top-level store path: no .., no trailing parts, no links
      [[ $out =~ ^/nix/store/[0-9a-df-np-sv-z]{32}-nixos-system-[A-Za-z0-9._+-]+$ ]] \
        || { echo "not a NixOS system path" >&2; exit 2; }
      [[ -d $out && ! -L $out ]] || { echo "not a directory" >&2; exit 2; }
      [[ -f $out/nixos-version && -x $out/bin/switch-to-configuration ]] \
        || { echo "not a built NixOS system" >&2; exit 2; }

      # registered in the store, so nothing half-written or put there by hand
      nix-store --check-validity "$out" 2>/dev/null \
        || { echo "not a valid store path" >&2; exit 2; }

      # who asked, for the journal: every activation leaves a line there
      logger -t emaki-nixos-activate -p auth.notice \
        "uid ''${PKEXEC_UID:-unknown} activates $out ($mode)"

      if [[ $mode != test ]]; then
        nix-env -p /nix/var/nix/profiles/system --set "$out"
      fi

      exec "$out/bin/switch-to-configuration" "$mode"
    '';
  };

  # the polkit action naming the helper's own store path, so pkexec runs exactly it; its
  # message carries the command line, so any polkit agent shows which system it's for
  policy = pkgs.writeTextFile {
    name = "emaki-nixos-activate-policy";
    destination = "/share/polkit-1/actions/org.emaki.nixos-activate.policy";
    text = ''
      <?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
        "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
      <policyconfig>
        <vendor>emaki</vendor>
        <action id="org.emaki.nixos-activate">
          <description>Apply a NixOS configuration</description>
          <message>Emaki Settings wants to apply system settings: $(command_line)</message>
          <icon_name>preferences-system</icon_name>
          <defaults>
            <allow_any>no</allow_any>
            <allow_inactive>no</allow_inactive>
            <allow_active>auth_admin</allow_active>
          </defaults>
          <annotate key="org.freedesktop.policykit.exec.path">${activate}/bin/emaki-nixos-activate</annotate>
        </action>
      </policyconfig>
    '';
  };
in
{
  options.programs.emaki.nixosActivate.enable = lib.mkEnableOption ''
    emaki-nixos-activate, the polkit-guarded helper Emaki Settings applies a built NixOS
    configuration with
  '';

  config = lib.mkIf cfg.enable {
    security.polkit.enable = true;
    environment.systemPackages = [
      activate
      policy
    ];
  };
}
