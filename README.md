# Wireguard Faketap

A small utility that adds a very basic version (i.e. single peer, no routing) of a regular wireguard implementation, but as a TAP device.
This is done mostly to facilitate networking tools working on the link-layer.

Since wireguard only works on L3, the ethernet link layer is emulated as necessary.

For the incoming side, the entire Ethernet header is simply stripped and whatever is incoming is sent out over the wireguard.
For the outgoing side, the EtherType is set to 0x0800 for IPv4 packets and 0x86DD for IPv6 packets.
MAC addresses are currently simply set to 00:00:00:00:00:00.

**DO NOT DIRECTLY CONNECT THE REGULAR ROUTING STACK TO THE TAP DEVICE i.e. DO NOT add an address to this interface.**

## Setup

You need effective `CAP_NET_ADMIN` for this to run.
In addition, due to a current code limitation, it calls `ip` as a subprocess, which also needs `CAP_NET_ADMIN`.
This means, that you usually need to be root.

A docker container that does the initial setup is provided, it needs the `NET_ADMIN` capability as well.

The following environment variables are used for config:

- `PRIVATE_KEY` specifies the private key of the wireguard
- `PEER_PUBLIC` for the public key of the single peer

In addition, the following environment variables can be used to customize the setup:

- `PEER_ENDPOINT` to specify and initial endpoint for sending packets to the peer.
If this is not specified, the address of the first incoming packet on the port will be used instead.
This will be confirmed by a println!().
- `LISTEN_ADDR` can be set to customize the socket listen address, otherwise defaulting to 0.0.0.0:51820
- `PERSISTENT_KEEPALIVE` can be set to customize the persistent keepalive period. The default is 25 (seconds).
