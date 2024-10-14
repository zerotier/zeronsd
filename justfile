build:
    nix build .#container

dns-ip network:
    @sudo zerotier-cli -j listnetworks \
        | jq -r \
            '.[] | select(.id == "{{network}}") \
            | .dns.servers \
            | select(.[] | test("\\d+\\.\\d+\\.\\d+\\.\\d+")) \
            | last'

run network config="./config.yaml" token="./.central.token": build
    file {{config}}
    sudo docker load < result
    sudo docker run \
      --net=host \
      --init \
      -v {{config}}:/var/lib/zeronsd/config.yaml \
      -v {{token}}:/var/lib/zeronsd/central.token \
      -v /var/lib/zerotier-one:/var/lib/zerotier-one \
      zeronsd:latest \
      zeronsd start -c /var/lib/zeronsd/config.yaml {{network}}
