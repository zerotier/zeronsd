VERSION := shell("toml get zeronsd/Cargo.toml package.version")
DOCKER_TAG := "zeronsd:" + VERSION

version:
    @echo {{VERSION}}

build:
    nix build

build-image:
    nix build .#container
    docker load < result
    docker tag {{DOCKER_TAG}} zeronsd:latest

resolve network name:
    nix run nixpkg#dig @$(zerotier-cli -j listnetworks \
        | jq -r \
            '.[] | select(.id == "{{network}}") \
            | .dns.servers \
            | select(.[] | test("\\d+\\.\\d+\\.\\d+\\.\\d+")) \
            | last') \
        {{name}}

run network config="./config.yaml" token="./.central.token": build
    file {{config}}
    sudo docker run \
      --net=host \
      --init \
      -v {{config}}:/var/lib/zeronsd/config.yaml \
      -v {{token}}:/var/lib/zeronsd/central.token \
      -v /var/lib/zerotier-one:/var/lib/zerotier-one \
      zerotier/{{DOCKER_TAG}} \
      zeronsd start -c /var/lib/zeronsd/config.yaml {{network}}
