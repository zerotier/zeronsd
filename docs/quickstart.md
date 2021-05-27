
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

* This is *very much* alpha software.
* This may end up integrated into ZeroTier 2.0, but for now, it is segregated to allow us to iterate quickly.
* Here be Dragons.

## Conceptual Prerequisites

* When ZeroTier joins a network, it creates a virtual network interface.
* When ZeroTier joins mutiple networks, there will be multiple network interfaces.
* When ZeroNSD starts, it binds to a ZeroTier network interface.
* When ZeroTier is joined to multiple networks, it need multiple ZeroNSDs, one for each interface.

This means:

* ZeroNSD will be accessible from the node it is running on.
* ZeroNSD will be accessible from other nodes on the ZeroTier network.
* ZeroNSD will be isolated from other networks the node might be on.

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
The following should work from the CLI on most plaforms. Windows users
may download the MSI from the [ZeroTier Downloads](https://www.zerotier.com/download/) page. For
the remainder of this document, please replace the example network `159924d630edb88e` with a network ID your own.

```
curl -s https://install.zerotier.com | bash
zerotier-cli join 159924d630edb88e
zerotier-cli  set 159924d630edb88e allowDNS=1
```

```
user@osx:~$ curl -s https://install.zerotier.com | sudo bash
user@osx:~$ zerotier-cli join 159924d630edb88e
user@osx:~$ zerotier-cli  set 159924d630edb88e allowDNS=1
```

## Authorize the Nodes

Authoriz the node to the network by clicking the "Auth" button in the
`Members` section in the
[ZeroTier Central WebUI](https://my.zerotier.com).

![Authorize the Member](https://i.imgur.com/fQTai9l.png)

## Provision a Central Token

Before we begin, we will need to log into [my.zerotier.com](https://my.zerotier.com) and create an API
token under the [Account](https://my.zerotier.com/account)
section. ZeroNSD will use this token to read Network members so it can
generate records, as well as update DNS settings.

![](https://i.imgur.com/WYM2jKl.png)

You will need to stash this in a file for ZeroNSD to read.

```
echo ZEROTIER_CENTRAL_TOKEN > ~/.token
chown zerotier-one:zerotier-one ~/.token
chmod 600 ~/.token
```

## ZeroTier Systemd Manager

ZeroTier Systemd Manager requires Go v1.16 or later. The instructions
below will install a working Go environment, allowing a `go get`.

We will be providing rpms and debs very soon.

```
curl -O https://storage.googleapis.com/golang/go1.16.4.linux-amd64.tar.gz
tar xzvf go1.16.4.linux-amd64.tar.gz -C /usr/local/
export GOPATH=/usr
export GOROOT=/usr/local/go
go get github.com/zerotier/zerotier-systemd-manager
```

We need to patch the `zerotier-one` unit, as we want `systemd-networkd`.

```
cat <<EOF> /lib/systemd/system/zerotier-one.service
[Unit]
Description=ZeroTier One
After=network.target
Wants=systemd-networkd.service

[Service]
ExecStart=/usr/sbin/zerotier-one
Restart=always
KillMode=process

[Install]
WantedBy=multi-user.target
EOF
```

Two more files for systemd.. timer and service units.

```
cat <<EOF> /lib/systemd/system/zerotier-systemd-manager.timer
[Unit]
Description=Update zerotier per-interface DNS settings
Requires=zerotier-systemd-manager.service

[Timer]
Unit=zerotier-systemd-manager.service
OnStartupSec=60
OnUnitInactiveSec=60
Persistent=true

[Install]
WantedBy=timers.target
EOF
```

```
cat <<EOF> /lib/systemd/system/zerotier-systemd-manager.service
[Unit]
Description=Update zerotier per-interface DNS settings
Wants=zerotier-systemd-manager.timer zerotier-one.service

[Service]
Type=oneshot
ExecStart=/usr/bin/zerotier-systemd-manager

[Install]
WantedBy=multi-user.target
EOF
```

Finally, restart all the ZeroTier services.

```
systemctl daemon-reload
systemctl restart zerotier-one
systemctl enable  zerotier-systemd-manager.timer
systemctl restart zerotier-systemd-manager.service
```

## Install ZeroNSD

ZeroNSD should only run on one node per network. Latency for DNS
really matters, so try to place it as close to the clients as
possible.

### Packages

ZeroNSD publishes rpm, deb, and msi packages, available at https://github.com/zerotier/zeronsd/releases

```
wget https://github.com/zerotier/zeronsd/releases/download/v0.1.4/zeronsd_0.1.4_amd64.deb
dpkg -i zeronsd_0.1.4_amd64.deb
```

### Cargo

If we don't have packages for your platform, you can install it with cargo.

```
/usr/bin/apt-get -y install net-tools librust-openssl-dev pkg-config cargo
/usr/bin/cargo install zeronsd --root /usr/local
```

## Serve DNS

For each network you want to serve DNS to, do the following

```
zeronsd supervise -t ~/.token -f /etc/hosts -d beyond.corp 159924d630edb88e
systemctl start zeronsd-159924d630edb88e
systemctl enable zeronsd-159924d630edb88e
```

## Verify functionality

You should be able to ping the laptop via it's DNS name.

```
root@ubuntu:~# ping laptop.beyond.corp
PING laptop.beyond.corp (172.22.192.177) 56(84) bytes of data.
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=1 ttl=64 time=50.1 ms
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=2 ttl=64 time=49.5 ms
64 bytes from 172.22.192.177 (172.22.192.177): icmp_seq=3 ttl=64 time=48.6 ms
```

Most Linux distributions, by default, do not have per-interface DNS
resolution out of the box. To test DNS queries against ZeroNSD without
`zerotier-systemd-manager`, find the IP address that ZeroNSD has bound
itself to, and run queries against it explicitly.

```
lsof -i -n | grep ^zeronsd | grep UDP | awk '{ print $9 }' | cut -f1 -d:
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
echo "1.2.3.4 test" >> /etc/hosts
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
