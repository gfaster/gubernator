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

- Manual wireguard
- Tailscale
- Nebula

Long and short is that wireguard is too hard and tailscale is too proprietary,
so I'm going to go with nebula.
