# Shell completion of host names
A quick and dirty way to get autocompletion of your zeronsd hostnames.
Comments and pull requests welcome.
You _don't_ need to be the admin of the network or of the zeronsd server.

- We use `nmap -sL` to list all the hostnames on a ZeroTier subnet.
- Write the names to files somewhere like ~/.config/hosts-$NETWORK_ID.
- Tell zsh or bash to use these files for host completion.

## (linux only) copy the zerotier authtoken.secret to your home directory. 
This lets you use zerotier-cli without sudo. 
On macOS the installer does this for you.

``` sh
sudo cp /var/lib/zerotier-one/authtoken.secret ~/.zeroTierOneAuthToken
sudo chown $(id -u):$(id -g) ~/.zeroTierOneAuthToken
```

## Run the script
The script will query your local zerotier-one for networks with DNS servers configured and create a file for each network.

The script depends on `jq` and `nmap`.

- `brew install jq nmap` or `apt install jq nmap` if you don't have them.
- `mkdir -p $HOME/.config/zeronsd`
- `chmod +x get-zeronsd-host-names.sh`
- `./get-zeronsd-host-names.sh ~/.config/zeronsd`

## Setup your shell to use the hosts
### zsh
Put this in your ~/.zshrc 

You may need to adapt it to your setup.

If you know a better way, let us know.

```sh
# get current hosts. zsh builtin stuff uses /etc/hosts, ~/.ssh/known_hosts, etc...
zstyle -s ':completion:*:hosts' hosts _hosts_config

# append hosts from zeronsd
[[ -r ~/.completion ]] && _hosts_config+=($(cat $HOME/.config/zeronsd/hosts-*))
zstyle ':completion:*:hosts' hosts $_hosts_config
```

### bash
Try this: https://blog.sanctum.geek.nz/bash-hostname-completion/

## Run nmap manually, without the script, if needed
Depending on your network setups, this may or may not fail. It's a rough shell script.

If you need to run it manually, it's basically:

`nmap -sL $SUBNET -oG - --dns-server=$SERVER | grep -v "()" | grep Host:  | cut -d "(" -f2 | cut -d ")" -f1 > $OUTDIR/hosts-$NETWORK_ID`

Where:
- $SUBNET :: Managed Route for the ZeroTier Network. For example: "10.147.20.0/24"
- $SERVER :: One of your zeronsd servers for this ZeroTier network. For example: "10.147.20.3"
- $NETWORK_ID :: The ZeroTier Network ID.

It's a small script. Edit it to your needs.

## Getting the script?
For now...
If you're reading this on github, click on the script then on "raw".
Then you can copy and paste it, or curl it from the current url, to somewhere like ~/bin/get-zeronsd-host-names.sh
