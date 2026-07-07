This document serves as both explanation and brainstorming

# Networking

The basic requirements for networking here are:

- Need all of our nodes to be able to communicate with each other directly
- Nodes can request IPs for their workloads
- Something about security that I haven't thought enough about yet

I am not very knowledgable about networking, so I will try to offload as much
of it as possible.

This means I need a VPN of some sort, probably a overlay network. There are 3
main possibilities here:

- Manual Wireguard
- Tailscale
- Nebula

Long and short is that Wireguard is too hard[^1] and Tailscale is too proprietary,
so I'm going to go with Nebula.

I should also have affordance for dealing with LAN ips as well, particularly
if I want to work without privileges or on Plan 9 (although it might not be too
bad to port Nebula to Plan 9 since [Tailscale pulled it off][tsp9]).

[^1]: This is mostly with regard to security and key management, as well as IP
    discovery.

[tsp9]: https://tailscale.com/blog/plan9-port
