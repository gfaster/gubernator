This document serves as both explanation and brainstorming.

# Artifacts

Jobs often need to read and write data, but the needs and methods may change.

For example, a build system might only need a url of a repository in some
cases, but in others (e.g. when you want to build your current worktree) it'll
want an entire, potentially large directory.

### Real-life examples

I want to be able to accommodate all of these at least (TODO: expand me)

- Jellyfin has a local database and media storage that may be mounted over the network
- A static web server could use mounted storage, or could be small enough that
  sending the files directly is fine.
- A build system needs a copy of repository, and needs to export the built objects
- A CD system needs built objects from the build system



## Methods for read-only files

Non-exhaustive strategies that I might be able to employ for managing files
that won't (can't) be changed by a job.

### Temporal locality

When dealing with files that will change relatively little between runs such as
with build systems, we might want to preference a machine that acted as a node
previously. We could provide a key that can be sent with a new job to allow
partial updates to those files (e.g. through rsync).

There will be other complications here with regards to locking or key reuse.
For example, what happens if we request two jobs that both use same key? In
other words, what if both jobs need different readonly files derivative of an
earlier job? (This is easy to functionally solve because you can just fallback
to a full transfer, but solving efficiently sounds like a fun problem).


### Files as part of the request

The simplest, potentially most reliable, but most limited method is including
the literal files as part of the job description. This requires no direct
communication between the requester and the node, so it could work well in
restrictive network environments. If there's reason to believe that everything
will need to be used, there might not be a significant advantage to using other
methods of sharing files outside of network pressure on the control server.

This would work well for config files.


### Files as a uri in the request

If it's accessible on the open internet, this is great. If it's a file on the
requesting machine, that might be a bit harder. 

One way I can handle this would be to request an ip to serve the files on the
host side, but that would require the requester to be able to join the mesh
network, which would make this method infeasible for http requests.

I could also combine this method with the last one, where the requester
directly uploads a file to the control server, which then forwards it to
another node which will in turn serve the files.


### Networked file systems (NFS, 9p)

If we have a ZFS system and root, we can share a dataset over NFS easily
(though auth is a can of worms that I do not understand fully). A simpler
solution is serving over 9p, which could be done easily in userspace, but
mounting it is more limited. If we can't mount with 9p, then we'd have to copy
everything anyway.

There's also SFTP, but I am unsure about transport considerations.



## Methods for exporting artifacts

This is specifically different from read+write, which I think I don't want to
support directly over the network (maybe instead this always has to go through
a service, such as with databases).

Exported artifacts here might be the "pure" output of a batch job (i.e. a
compiled binary), or it could be the modification of a file we received in a
read-only way, in which case it's like the service receiving a mutex lock on
those files.


### Sending the files as part of status

A lot of the same issues as before, except I suspect that exported artifacts
will often be larger than their inputs.


### Serving files as networked file systems (NFS, 9p)

Good for if future jobs would want the artifact since we know nodes have access
to the mesh network. This would also require lingering storage usage on a node.

Again, it might be nice to serve ZFS datasets over NFS.
