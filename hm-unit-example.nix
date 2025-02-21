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
  # enable with "veridian-controller.enable = true;" in home-manager config
  options."${moduleName}".enable = lib.mkEnableOption "Enable ${description}";

  config = lib.mkIf config."${moduleName}".enable {
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
