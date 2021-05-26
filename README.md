# ZeroNS: a name service centered around the ZeroTier Central API

ZeroNS provides names that are a part of [ZeroTier Central's](https://my.zerotier.com) configured _networks_; once provided an IPv4-capable network it:

- Listens on the local interface joined to that network -- you will want to start one ZeroNS per ZeroTier network.
- Provides general DNS by forwarding all queries to `/etc/resolv.conf` resolvers that do not match the TLD, similar to `dnsmasq`.
- Tells Central to point all clients that have the "Manage DNS" settings turned **on** to resolve to it.
- Finally, sets a provided TLD (`.domain` is the default), as well as configuring `A` (IPv4) records for:
  - Member IDs: `zt-<memberid>.<tld>` will resolve to the IPv4 addresses for them.
  - Names: _if_ the names are compatible with DNS names, they will be converted as such: to `<name>.<tld>`.
    - Please note that **collisions are possible** and that it's _up to the admin to prevent them_.

_Please note that zeronsd still does not properly utilize IPv6; we hope to have this resolved by 0.2.0._

## Installation

Packages:

- Linux/Windows: [releases](https://github.com/zerotier/zeronsd/releases) contain packages for `*.deb`, `*.rpm` for Linux, and MSI format for Windows.
- Mac OS X: `brew tap zerotier/homebrew-tap && brew install zerotier/homebrew-tap/zeronsd`
- Docker: `docker pull zerotier/zeronsd` (see below for more on docker)

Other methods:

### Get a release from Cargo

Please obtain a working [rust environment](https://rustup.rs/) first.

```
cargo install zeronsd
```

### From Git

```
cargo install --git https://github.com/zerotier/zeronsd --branch main
```

### Docker

There is a `Dockerfile` present in the repository you can use to build images in lieu of one of our [official images](https://hub.docker.com/r/zerotier/zeronsd).

There are build arguments which control behavior:

- `IS_LOCAL`: if set, uses the local source tree and does not try to fetch.
- `VERSION`: this is the branch or tag to fetch.
- `IS_TAG`: if non-zero, tells cargo to fetch tags instead of branches.

Example:

```bash
docker build . # builds latest master
docker build --build-arg VERSION=somebranch # builds branch `somebranch`
docker build --build-arg IS_TAG=1 --build-arg VERSION=v0.1.0 # builds version 0.1.0 from tag v0.1.0
```

Once built, the image automatically runs `zeronsd` for you. The default subcommand is `help`.

## Usage

Setting `ZEROTIER_CENTRAL_TOKEN` in the environment is required. You must be able to administer the network to use `zeronsd` with it. Also, running as `root` is required as _many client resolvers do not work over anything but port 53_. Your `zeronsd` instance will listen on both `udp` and `tcp`, port `53`.

### Bare commandline

**Tip**: running `sudo`? Pass the `-E` flag to import your current shell's environment, making it easier to add the `ZEROTIER_CENTRAL_TOKEN`.

```
zeronsd start <network id>
```

### Running as a service

_This behavior is currently only supported on Linux; we will accept patches for other platforms._

The `zeronsd supervise` and `zeronsd unsupervise` commands can be used to manipulate systemd unit files related to your network. For the `supervise` case, simply pass the arguments you would normally pass to `start` and it will generate a unit from it.

Example:

```bash
# to enable
zeronsd supervise -t ~/.token -f /etc/hosts -d mydomain 36579ad8f6a82ad3
# generates systemd unit file named /lib/systemd/system/zeronsd-36579ad8f6a82ad3.service
systemctl daemon-reload
systemctl enable zeronsd-36579ad8f6a82ad3.service && systemctl start zeronsd-36579ad8f6a82ad3.service

# to disable
systemctl disable zeronsd-36579ad8f6a82ad3.service && systemctl stop zeronsd-36579ad8f6a82ad3.service
zeronsd unsupervise 36579ad8f6a82ad3
systemctl daemon-reload
```

### Docker

Running in docker is a little more complicated. You must be able to have a network interface you can import (joined a network) and must be able to reach `localhost:9999` on the host. At this time, for brevity's sake we are recommending running with `--net=host` until we have more time to investigate a potentially more secure solution.

You also need to mount your `authtoken.secret`, which we use to talk to `zerotier-one`

```

docker run -v /var/lib/zerotier-one:/var/lib/zerotier-one:ro --net host zerotier/zeronsd start <network id>

```

### Other notes

You must have already joined a network and obviously, `zerotier-one` should be running!

It should print some diagnostics after it has talked to your `zerotier-one` instance to figure out what IP to listen on. After that it should communicate with the central API and set everything else up automatically.

### Flags

- `-d <tld>` will set a TLD for your records; the default is `domain`.
- `-f <hosts file>` will parse a file in `/etc/hosts` format and append it to your records.
- `-s <secret file>` path to `authtoken.secret` which is needed to talk to ZeroTier on localhost. You can provide this file with this argument, but it is auto-detected on multiple platforms including Linux, OS X and Windows.
- `-t <central token file>` path to file containing your [ZeroTier Central token](https://my.zerotier.com/account).

### TTLs

Records currently have a TTL of 60s, and Central's records are refreshed every 30s through the API. I felt this was a safer bet than letting timeouts happen.

### Per-Interface DNS resolution

OS X and Windows users get this functionality by default, so there is no need for it.

Linux users are strongly encouraged to use `systemd-networkd` along with `systemd-resolved` to get per-interface resolvers that you can isolate to the domain you want to use. If you'd like to try something that can assist with getting you going quickly, check out the [zerotier-systemd-manager repository](https://github.com/zerotier/zerotier-systemd-manager).

BSD systems still need a bit of work; work that we could really use your help with if you know the lay of the land on your BSD of choice. Set up an issue if this interests you.

## Acknowledgements

ZeroNS demands a lot out of the [trust-dns](https://github.com/bluejekyll/trust-dns) toolkit and I personally am grateful such a library suite exists. It made my job very easy.

## Author

Erik Hollensbe <github@hollensbe.org>

```

```
