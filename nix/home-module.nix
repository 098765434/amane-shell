# emaki's user services, for home-manager: `imports = [ inputs.emaki.homeManagerModules.default ];`
# then `programs.emaki.enable = true;` (and `.drop.enable`, `.assistant.enable` for those).
#
# How much each is sandboxed follows from what it starts:
#
# - The shell and Drop open apps. An app opened from the dock or a Drop tile is the shell's (or
#   Drop's) child before amane moves it into a scope of its own, and a child keeps its parent's
#   namespaces and no-new-privileges: a sandboxed shell would hand every app it opens a
#   read-only system, no setuid (no sudo, no pkexec) and the rest. So these two get only what
#   no child inherits in a way it would notice: no core dumps of them, and restarting.
# - The assistant's daemon opens nothing, so it gets every restriction that needs no namespace:
#   no new privileges, no network (only unix sockets), a system-call filter, no core dumps. The
#   ones that do need one (ProtectSystem, ProtectHome, PrivateTmp, PrivateNetwork…) are left
#   out: in a user unit they put the daemon in a user namespace of its own, from which it can't
#   read /proc/<pid>/exe of the program connecting, so it couldn't check that it's the shell, and
#   any program could take its microphone. It confines its own file access with Landlock instead
#   (apps/assistant/src/main.rs), which needs no namespace.
self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.emaki;
  package = cfg.package;

  # for a unit started with the session, and ended with it
  session = {
    PartOf = [ "graphical-session.target" ];
    After = [ "graphical-session.target" ];
  };

  # what a daemon that starts nothing can be held to, with no namespace (see the top)
  sandbox = {
    NoNewPrivileges = true;
    LockPersonality = true;
    RestrictRealtime = true;
    RestrictSUIDSGID = true;
    RestrictNamespaces = true;
    SystemCallArchitectures = "native";
    SystemCallFilter = [
      "@system-service"
      "~@privileged"
    ];
    # a call left out fails rather than killing the daemon
    SystemCallErrorNumber = "EPERM";
    KeyringMode = "private";
    UMask = "0077";
    LimitCORE = 0;
  };
in
{
  options.programs.emaki = {
    enable = lib.mkEnableOption "emaki, the shell, as a user service";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "emaki.packages.\${system}.default";
      description = "The emaki package: the shell and the programs beside it.";
    };

    drop.enable = lib.mkEnableOption "Drop (emaki-drop) as a user service of its own";

    assistant.enable = lib.mkEnableOption ''
      the assistant's daemon (emaki-assistant), started by its socket when the shell first asks
    '';
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ package ];

    systemd.user.services.emaki = {
      Unit = session // {
        Description = "emaki, the desktop shell";
      };
      Service = {
        ExecStart = "${package}/bin/emaki";
        Restart = "on-failure";
        # its apps inherit nothing else from the unit (see the top of this file); the shell
        # turns its own core dumps off itself
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };

    # a unit of its own, outside the shell's cgroup, so a restart of the shell doesn't take it
    # down; it puts its item back once the bar is there
    systemd.user.services.emaki-drop = lib.mkIf cfg.drop.enable {
      Unit = session // {
        Description = "emaki-drop (Drop: a grid of folders, apps and uploads)";
      };
      Service = {
        ExecStart = "${package}/bin/emaki-drop";
        Restart = "on-failure";
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };

    systemd.user.sockets.emaki-assistant = lib.mkIf cfg.assistant.enable {
      Unit = session // {
        Description = "emaki-assistant's socket";
      };
      Socket = {
        ListenStream = "%t/emaki-assistant.sock";
        # only this user's programs; the daemon also takes only the shell of its own build
        SocketMode = "0600";
        DirectoryMode = "0700";
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };

    systemd.user.services.emaki-assistant = lib.mkIf cfg.assistant.enable {
      Unit = session // {
        Description = "emaki-assistant, the launcher's assistant";
        Requires = [ "emaki-assistant.socket" ];
      };
      Service = sandbox // {
        ExecStart = "${package}/bin/emaki-assistant";
        # Cactus has no network code; the daemon may only make unix sockets (its own, and the
        # sound server's)
        RestrictAddressFamilies = [ "AF_UNIX" ];
        # its tool index, made and left writable; the models are read from the store or the
        # data folder
        CacheDirectory = "emaki-assistant";
        # its models take 80–140 MB; a runaway is stopped long before the machine is
        MemoryMax = "1G";
      };
    };
  };
}
