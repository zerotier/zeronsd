# ZeroNS: a name service centered around the ZeroTier Central API

ZeroNS provides names that are a part of Central's configured _networks_; once provided a network it:

- Listens on the local interface joined to that network -- you will want to start one ZeroNS per ZeroTier network.
- Provides general DNS by forwarding all queries to `/etc/resolv.conf` resolvers that do not match the TLD, similar to `dnsmasq`.
- Tells Central to point all clients that have the "Manage DNS" settings turned **on** to resolve to it.
- Finally, sets a provided TLD (`.domain` is the default), as well as configuring `A` (IPv4) and `AAAA` (IPv6) records for:
  - Member IDs: `zt-<memberid>.<tld>` will resolve to the IPv4/v6 addresses for them.
  - Names: _if_ the names are compatible with DNS names, they will be converted as such: to `<name>.<tld>`.
    - Please note that **collisions are possible** and that it's _up to the admin to prevent them_.

## Installation

Please obtain a working [rust environment](https://rustup.rs/) first.

```
cargo install --git https://github.com/erikh/zeronsd --branch main
```

## Usage

Setting `ZEROTIER_CENTRAL_TOKEN` in the environment is required. You must be able to administer the network to use `zeronsd` with it. Also, running as `root` is required as _many client resolvers do not work over anything but port 53_. Your `zeronsd` instance will listen on both `udp` and `tcp`, port `53`.

**Tip**: running `sudo`? Pass the `-E` flag to import your current shell's environment, making it easier to add the `ZEROTIER_CENTRAL_TOKEN`.

```
zeronsd start <network id>
```

You must have already joined a network and obviously, `zerotier-one` should be running!

It should print some diagnostics after it has talked to your `zerotier-one` instance to figure out what IP to listen on. After that it should communicate with the central API and set everything else up automatically.

### Flags

- `-d <tld>` will set a TLD for your records; the default is `domain`.
- `-f <hosts file` will parse a file in `/etc/hosts` format and append it to your records.

### TTLs

Records currently have a TTL of 60s, and Central's records are refreshed every 30s through the API. I felt this was a safer bet than letting timeouts happen.

## Acknowledgements

ZeroNS demands a lot out of the [trust-dns](https://github.com/bluejekyll/trust-dns) toolkit and I personally am grateful such a library suite exists. It made my job very easy.

## Author

Erik Hollensbe <github@hollensbe.org>
