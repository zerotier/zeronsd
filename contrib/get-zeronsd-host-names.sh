#!/usr/bin/env bash
set -euo pipefail


# use $HOME/hosts/ as the directory, unless user specifies something else
if [ $# -eq 0 ];
then
    OUTDIR="$HOME/.hosts"
else
    OUTDIR=$1
fi

# Get list of Network IDs that have DNS enabled
NWIDS=$(zerotier-cli listnetworks -j | jq -r ".[] | select(.dns.servers?) | .id")

mkdir -p $OUTDIR

# clear hosts from old, unjoined zerotier networks
# rm -f $OUTDIR/hosts*

for NWID in $NWIDS
do
    # get one of the DNS server addresses
    SERVER=$(zerotier-cli listnetworks -j | jq -r ".[] | select(.id == \"$NWID\") | .dns | .servers[0]")

    # Get the subnet/cidr of the zerotier network
    SUBNET=$(zerotier-cli listnetworks -j | jq -r ".[] | select(.id == \"$NWID\") | .routes | .[] | select(.via == null) | .target")

    # scan each network with nmap and output names to file in $OUTDIR
    nmap -sL $SUBNET -oG - --dns-server=$SERVER | grep -v "()" | grep Host:  | cut -d "(" -f2 | cut -d ")" -f1 > $OUTDIR/hosts-$NWID

    # echo "Wrote host names to: $OUTDIR/hosts-$NWID"
done
