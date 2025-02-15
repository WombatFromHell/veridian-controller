{
  lib,
  pkgs,
  config,
  osConfig ? {},
  ...
}: let
  moduleName = "veridian-controller";
  description = "Veridian Controller Fan Service";
in {
  options."${moduleName}".enable = lib.mkEnableOption "Enable ${description}";

  config = lib.mkIf config."${moduleName}".enable {
    # nixos unit example
    environment.systemPackages = [pkgs.${moduleName}];
    systemd = {
      # ensure the config file exists with the right perms
      tmpfiles.rules = [
        "f /etc/veridian-controller.toml 0640 root root -"
      ];
      services."${moduleName}" = {
        description = "${description}";
        wantedBy = ["multi-user.target"];
        serviceConfig = {
          Environment = "PATH=/run/wrappers/bin:${config.hardware.nvidia.package.settings}/bin:${config.hardware.nvidia.package.bin}/bin";
          Type = "simple";
          ExecStart = "${pkgs.${moduleName}}/bin/${moduleName} -f /etc/veridian-controller.toml";
          TimeoutStopSec = 10;
        };
      };
    };

    # home-manager unit example
    home.packages = [pkgs.${moduleName}];
    systemd.user.services."${moduleName}" = {
      Unit = {
        Description = "${description}";
      };
      Service = {
        # expose the wrapped sudo, nvidia-settings, and nvidia-smi utils
        Environment = "PATH=/run/wrappers/bin:${osConfig.hardware.nvidia.package.settings}/bin:${osConfig.hardware.nvidia.package.bin}/bin";
        Type = "simple";
        ExecStart = "${pkgs.${moduleName}}/bin/${moduleName}";
        TimeoutStopSec = 10;
      };
      Install = {
        WantedBy = ["graphical-session.target"];
      };
    };
  };
}
