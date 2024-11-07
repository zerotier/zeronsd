VERSION := shell("toml get zeronsd/Cargo.toml package.version")
DOCKER_TAG := "zeronsd:" + VERSION
DOCKER_BIN := env_var_or_default("DOCKER_BIN", "docker")

version:
    @echo {{VERSION}}

docker:
    @echo {{DOCKER_BIN}}

build:
    nix build

docker-build platform="linux/arm64,linux/amd64":
    {{DOCKER_BIN}} build \
        --platform={{platform}} \
        -t {{DOCKER_TAG}} \
        .

clean-image:
    {{DOCKER_BIN}} image rm {{DOCKER_TAG}}

resolve network name:
    nix run nixpkg#dig @$(zerotier-cli -j listnetworks \
        | jq -r \
            '.[] | select(.id == "{{network}}") \
            | .dns.servers \
            | select(.[] | test("\\d+\\.\\d+\\.\\d+\\.\\d+")) \
            | last') \
        {{name}}

run network config="./config.yaml" token="./.central.token" docker-bin="docker": build
    file {{config}}
    sudo {{DOCKER_BIN}} run \
      --net=host \
      --init \
      -v {{config}}:/var/lib/zeronsd/config.yaml \
      -v {{token}}:/var/lib/zeronsd/central.token \
      -v /var/lib/zerotier-one:/var/lib/zerotier-one \
      zerotier/{{DOCKER_TAG}} \
      zeronsd start -c /var/lib/zeronsd/config.yaml {{network}}
