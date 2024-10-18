{ pkgs, cargo-package, ... }:

pkgs.dockerTools.buildImage {
  name = cargo-package.name;
  tag = cargo-package.version;
  created = "now";

  copyToRoot = pkgs.buildEnv {
    name = "zeronsd-image-root";
    paths = with pkgs; [ zeronsd zerotierone dockerTools.caCertificates ];
    pathsToLink = [ "/bin" "/etc" ];
  };

  config = {
    Cmd = [ "/bin/zeronsd" ];
    WorkingDir = "/var/lib/zeronsd";
    Volumes = { "/var/lib/zeronsd" = {}; };
  };
}
