{
  lib,
  pkgs,
  config,
  ...
}: let
  moduleName = "veridian-controller";
  description = "Veridian Controller User Fan Service";
in {
  # enable with "services.veridian-controller.enable = true;" in nixos config
  options.services."${moduleName}".enable = lib.mkEnableOption "Enable ${description}";

  config = lib.mkIf config.services."${moduleName}".enable {
    # nixos unit example
    environment.systemPackages = [pkgs.${moduleName}];
    # ensure the config file exists with the right perms
    systemd = {
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
  };
}
