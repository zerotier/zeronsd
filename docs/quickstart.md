# ZeroNSD Quickstart

<p align="center">
<img src="https://avatars.githubusercontent.com/u/4173285?s=200&v=4" alt="ZeroNSD" style="width:100%;"><br>
<b><i>
It's not DNS.<br>
There's no way it's DNS.<br>
It was DNS.<br>
</i></b>
</p>

## Status

- This feature is still in beta.
- This will soon be integrated into ZeroTier 2.0, but for now, it is segregated to allow us to iterate quickly.
- Here be Dragons (still).

## Conceptual Prerequisites

- When ZeroTier joins a network, it creates a virtual network interface.
- When ZeroTier joins mutiple networks, there will be multiple network interfaces.
- When ZeroNSD starts, it binds to a ZeroTier network interface.
- When ZeroTier is joined to multiple networks, it needs multiple ZeroNSDs, one for each interface.

This means:

- ZeroNSD will be accessible from the node it is running on.
- ZeroNSD will be accessible from other nodes on the ZeroTier network.
- ZeroNSD will be isolated from other networks the node might be on.

## Technical Prerequisites

This Quickstart was written using two machines - one Ubuntu virtual
machine on Digital Ocean, and one OSX laptop on a residential ISP. To
follow along step by step, you'll need to provision equivalent
infrastructure. If you use different platforms, you should be able to
figure out what to do with minimal effort.

## Create a ZeroTier Network

You may do this manually through the [ZeroTier Central WebUI](https://my.zerotier.com),

![Create a Network](https://i.imgur.com/L6xtGKo.png)

## Install ZeroTier

ZeroTier must be installed and joined to the network you intend to provide DNS service to.
The following should work from the CLI on most platforms. Windows users
may download the MSI from the [ZeroTier Downloads](https://www.zerotier.com/download/) page. For
the remainder of this document, please replace the example network `af78bf94364e2035` with a network ID your own.

```
notroot@ubuntu:~$ curl -s https://install.zerotier.com | sudo bash
notroot@ubuntu:~$ sudo zerotier-cli join af78bf94364e2035
notroot@ubuntu:~$ sudo zerotier-cli  set af78bf94364e2035 allowDNS=1
```

## Authorize the Nodes

Authorize the node to the network by clicking the "Auth" button in the
`Members` section in the
[ZeroTier Central WebUI](https://my.zerotier.com).

![Authorize the Member](https://i.imgur.com/fQTai9l.png)

## Provision an API Token from ZeroTier Central

Before we begin, we will need to log into [my.zerotier.com](https://my.zerotier.com) and create an API
token under the [Account](https://my.zerotier.com/account)
section. ZeroNSD will use this token to read Network members so it can
generate records, as well as update DNS settings.

![](https://i.imgur.com/WYM2jKl.png)

You will need to stash this in a file for ZeroNSD to read.

```
sudo bash -c "echo ZEROTIER_CENTRAL_TOKEN > /var/lib/zerotier-one/token"
sudo chown zerotier-one:zerotier-one /var/lib/zerotier-one/token
sudo chmod 600 /var/lib/zerotier-one/token
```

## ZeroTier Systemd Manager

zerotier-systemd-manager publishes rpm and deb packages available at https://github.com/zerotier/zerotier-systemd-manager/releases

```
wget https://github.com/zerotier/zerotier-systemd-manager/releases/download/v0.1.9/zerotier-systemd-manager_0.1.9_linux_amd64.deb
sudo dpkg -i zerotier-systemd-manager_0.1.9_linux_amd64.deb
```

Finally, restart all the ZeroTier services.

```
sudo systemctl daemon-reload
sudo systemctl restart zerotier-one
sudo systemctl enable zerotier-systemd-manager.timer
sudo systemctl start zerotier-systemd-manager.timer
```

## Install ZeroNSD

ZeroNSD should only run on one node per network. Latency for DNS
really matters, so try to place it as close to the clients as
possible.

### Packages

ZeroNSD publishes rpm, deb, and msi packages, available **[here](https://github.com/zerotier/zeronsd/releases)**.

_The latest release is **not** reflected below. Go to the link above to get a link!_

```
wget https://github.com/zerotier/zeronsd/releases/download/v0.1.7/zeronsd_0.1.7_amd64.deb
sudo dpkg -i zeronsd_0.1.7_amd64.deb
```

### Cargo

If we don't have packages for your platform, you can still install it with cargo.

```
sudo /usr/bin/apt-get -y install net-tools librust-openssl-dev pkg-config cargo
sudo /usr/bin/cargo install zeronsd --root /usr/local
```

## Serve DNS

For each network you want to serve DNS to, do the following (replace `af78bf94364e2035` with *your* network ID)

```
sudo zeronsd supervise -t /var/lib/zerotier-one/token -w -d beyond.corp af78bf94364e2035
sudo systemctl start zeronsd-af78bf94364e2035
sudo systemctl enable zeronsd-af78bf94364e2035
```

## Verify functionality

You should be able to ping the laptop via it's DNS name (or any preceding subdomain, since we've set the wildcard flag)

```
notroot@ubuntu:~$ ping laptop.beyond.corp
PING laptop.beyond.corp (172.22.192.177) 56(84) bytes of data.
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=1 ttl=64 time=50.1 ms
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=2 ttl=64 time=49.5 ms
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=3 ttl=64 time=48.6 ms
```
or
```
notroot@ubuntu:~$ ping laptop.beyond.corp
PING travel.laptop.beyond.corp (172.22.192.177) 56(84) bytes of data.
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=1 ttl=64 time=50.1 ms
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=2 ttl=64 time=49.5 ms
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=3 ttl=64 time=48.6 ms
```

## Update flag settings

In order to change the settings (such as the TLD), do the following (replace `af78bf94364e2035` with *your* network ID)
```
sudo zeronsd supervise -t /var/lib/zerotier-one/token -w -d beyond.corp af78bf94364e2035
sudo systemctl daemon-reload
sudo systemctl enable zeronsd-af78bf94364e2035
```

Most Linux distributions, by default, do not have per-interface DNS
resolution out of the box. To test DNS queries against ZeroNSD without
`zerotier-systemd-manager`, find the IP address that ZeroNSD has bound
itself to, and run queries against it explicitly.

```
sudo lsof -i -n | grep ^zeronsd | grep UDP | awk '{ print $9 }' | cut -f1 -d:
172.22.245.70
```

Query the DNS server directly with the dig command

The Ubuntu machine can be queried with:

```
dig +short @172.22.245.70 zt-3513e8b98d.beyond.corp
172.22.245.70
dig +short @172.22.245.70 server.beyond.corp
172.22.245.70
```

The OSX laptop can be queried with:

```
dig +short @172.22.245.70 zt-eff05def90.beyond.corp
172.22.245.70
dig +short @172.22.245.70 laptop.beyond.corp
172.22.192.177
```

Add a line to `/etc/hosts` and query again.

```
bash -c 'echo "1.2.3.4 test" >> /etc/hosts'
dig +short @172.22.245.70 test.beyond.corp
1.2.3.4
```

Query a domain on the public DNS to verify fall through

```
dig +short @172.22.245.70 example.com
93.184.216.34
```

## OSX

OSX uses `dns-sd` for DNS resolution. Unfortunately, `nslookup`,`host`, and `dig` are broken on OSX.  
`ping` works.

```
user@osx:~$ ping server.beyond.corp
PING server.beyond.corp (172.22.245.70): 56 data bytes
64 bytes from 172.22.245.70: icmp_seq=0 ttl=64 time=37.361 ms
64 bytes from 172.22.245.70: icmp_seq=1 ttl=64 time=38.129 ms
64 bytes from 172.22.245.70: icmp_seq=2 ttl=64 time=37.569 ms
```

To check out the system resolver settings, use: `scutil --dns`.

The Ubuntu machine can be queried with

`dns-sd -G v4 server.beyond.corp`  
`dns-sd -G v4 zt-3513e8b98d.beyond.corp`

The OSX machine be queried with

`dns-sd -G v4 laptop.beyond.corp`  
`dns-sd -G v4 zt-eff05def90.beyond.corp`

## Windows

Are you a Windows user?  
Does this work out of the box?  
Does nslookup behave properly?  
Let us know... feedback and pull requests welcome =)

## Serving non-ZeroTier records

**NOTE** this portion of the document is largely intended for advanced users who want to get more out of `zeronsd`'s service.

`zeronsd` will also serve non-zerotier records in two situations: It will forward `/etc/resolv.conf`'s nameservers on a TLD mismatch. This behavior is similar to `dnsmasq`, a popular DNS server on Linux.

Additionally, to serve custom records you can supply the `-f` flag with a file in [hosts format](https://man7.org/linux/man-pages/man5/hosts.5.html) it will service records from that file under the provided TLD, _merged in_ with the zerotier nodes. Example below.

**NOTE:** if you followed the steps above, you will want to `systemctl stop zeronsd-<network id>`, and `zeronsd unsupervise <network id>` your network, before continuing.

Make a file called `hosts` and put this in it:

```
1.1.1.1 cloudflare-dns
```

Then, let's start a temporary server for now. We'll just use the `start` subcommand of `zeronsd`. This will run in the foreground, so start a new terminal or `&` it.

```
$ zeronsd start -t /var/lib/zerotier-one/token -f ./hosts -d beyond.corp <network id>
Welcome to ZeroNS!
Your IP is 1.2.3.4
```

Finally, we can lookup `cloudflare-dns.beyond.corp` to find CloudFlare's DNS server really really fast!

```
$ host cloudflare-dns.beyond.corp 1.2.3.4
cloudflare-dns.beyond.corp has address 1.1.1.1
```
