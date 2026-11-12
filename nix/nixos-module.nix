# The system half of emaki: the root-run helpers Emaki Settings uses from the session, through
# polkit. Import it from your configuration's flake (`emaki.nixosModules.default`) and set:
#
#   programs.emaki.nixosActivate.enable = true;      # applies built NixOS configurations
#   programs.emaki.chargeThreshold.enable = true;    # sets BAT*/charge_control_*_threshold
#
# Settings builds the configuration as you and only hands the activate helper a store path; root
# never evaluates the repo. Anything that runs as you can call `pkexec emaki-nixos-activate` too,
# so the helper is strict about what it activates, every call asks for the password (no
# auth_admin_keep), and the prompt shows the store path it's for. Look at it before you type
# your password: a path you didn't just build in Settings is not yours.
#
# The charge-threshold helper is low-risk (two integers 0-100 that firmware clamps to safe
# ranges) and runs without a password for the active session; the helper still validates argv
# and writes to a strict glob, so it can't be redirected elsewhere.
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
  activateCfg = config.programs.emaki.nixosActivate;
  chargeCfg = config.programs.emaki.chargeThreshold;
  fprintCfg = config.programs.emaki.fingerprint;
  firmwareCfg = config.programs.emaki.firmware;

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
  activatePolicy = pkgs.writeTextFile {
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

  # Writes BAT*/charge_control_{start,end}_threshold on ThinkPads and compatible firmware that
  # expose them. Argv is two integers 0-100 with start <= end; the helper never builds a path
  # from argv, only iterates the sysfs glob, so a planted "../" or absolute path is impossible.
  chargeThreshold = pkgs.writeShellApplication {
    name = "emaki-charge-threshold";
    runtimeInputs = [ pkgs.coreutils ];
    text = ''
      # emaki-charge-threshold <start:0-100> <end:0-100>
      umask 077

      if [[ $# -ne 2 ]]; then
        echo "usage: emaki-charge-threshold <start:0-100> <end:0-100>" >&2
        exit 2
      fi

      start=$1
      end=$2

      [[ $start =~ ^(0|[1-9][0-9]?|100)$ ]] || { echo "start not in 0-100" >&2; exit 2; }
      [[ $end =~ ^(0|[1-9][0-9]?|100)$ ]]   || { echo "end not in 0-100"   >&2; exit 2; }
      if (( start > end )); then
        echo "start > end" >&2
        exit 2
      fi

      # a strict glob with no argv interpolation; a battery that lacks either file is skipped
      # (not every battery supports both thresholds), and finding none is itself a refusal
      wrote=0
      shopt -s nullglob
      for start_file in /sys/class/power_supply/BAT*/charge_control_start_threshold; do
        end_file=''${start_file%start_threshold}end_threshold
        [[ -f $end_file ]] || continue

        # Ordering matters: when the new start is above the current end the kernel rejects it,
        # and the other way round. Write end-then-start when raising, start-then-end when
        # lowering, so a mid-range change goes through on the first try.
        current_end=$(<"$end_file")
        if (( end >= current_end )); then
          printf %s "$end"   > "$end_file"
          printf %s "$start" > "$start_file"
        else
          printf %s "$start" > "$start_file"
          printf %s "$end"   > "$end_file"
        fi
        wrote=1
      done

      if (( wrote == 0 )); then
        echo "no battery supports charge thresholds on this machine" >&2
        exit 3
      fi
    '';
  };

  chargeThresholdPolicy = pkgs.writeTextFile {
    name = "emaki-charge-threshold-policy";
    destination = "/share/polkit-1/actions/org.emaki.charge-threshold.policy";
    text = ''
      <?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
        "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
      <policyconfig>
        <vendor>emaki</vendor>
        <action id="org.emaki.charge-threshold">
          <description>Set the battery's charge thresholds</description>
          <message>Emaki Settings wants to change when the battery charges.</message>
          <icon_name>battery</icon_name>
          <defaults>
            <allow_any>no</allow_any>
            <allow_inactive>no</allow_inactive>
            <allow_active>yes</allow_active>
          </defaults>
          <annotate key="org.freedesktop.policykit.exec.path">${chargeThreshold}/bin/emaki-charge-threshold</annotate>
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

  options.programs.emaki.chargeThreshold.enable = lib.mkEnableOption ''
    emaki-charge-threshold, the polkit-guarded helper Emaki Settings writes the battery's
    charge_control_{start,end}_threshold through. Enable on laptops whose firmware exposes
    these (ThinkPad, most Framework and Dell models)
  '';

  options.programs.emaki.fingerprint = {
    enable = lib.mkEnableOption ''
      fprintd and the PAM wiring Emaki Settings' Security page needs to enrol, delete and
      verify fingerprints. Turns on services.fprintd and asks swaylock to accept a finger
      instead of a password at the lock screen
    '';

    swaylockUnlock = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Wires pam_fprintd into swaylock, so touching the reader unlocks the screen. Leave on
        unless you want the enrol UI without the unlock path (for example, if some other PAM
        stack already asks for the finger).
      '';
    };
  };

  options.programs.emaki.firmware.enable = lib.mkEnableOption ''
    fwupd, the firmware updater Emaki Settings' Firmware page reads and runs. Turns on
    services.fwupd so devices (UEFI, EC, Thunderbolt, docks) can receive LVFS updates; the
    Settings page shells out to fwupdmgr, which handles its own polkit prompts. Enable the
    lvfs-testing remote with services.fwupd.extraRemotes in your own configuration if you
    want firmware in testing
  '';

  config = lib.mkMerge [
    (lib.mkIf activateCfg.enable {
      security.polkit.enable = true;
      environment.systemPackages = [
        activate
        activatePolicy
      ];
    })
    (lib.mkIf chargeCfg.enable {
      security.polkit.enable = true;
      environment.systemPackages = [
        chargeThreshold
        chargeThresholdPolicy
      ];
    })
    (lib.mkIf fprintCfg.enable {
      services.fprintd.enable = true;
      # swaylock's PAM stack is used by the shell's lock-session path; adding fprintAuth
      # lets the reader stand in for the password. It doesn't disable the password: swaylock
      # still accepts one if the reader is unplugged, mis-reads or is disabled by the user.
      security.pam.services.swaylock.fprintAuth = lib.mkIf fprintCfg.swaylockUnlock true;
    })
    (lib.mkIf firmwareCfg.enable {
      services.fwupd.enable = true;
    })
  ];
}
