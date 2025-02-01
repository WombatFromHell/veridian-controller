# include this module in your home-manager config
{
  pkgs, # make sure to include the input from your flake
  ...
}: let
  moduleName = "veridian-controller";
  description = "Veridian Controller User Fan Service";
in {
  # systemd user service to start the fan controller on startup
  systemd.user.services."${moduleName}" = {
    Unit = {
      Description = "${description}";
    };
    Service = {
      Type = "simple";
      ExecStart = "${pkgs.${moduleName}}/bin/${moduleName}";
    };
    Install = {
      WantedBy = ["graphical-session.target"];
    };
  };
}
