## Goals

The primary use-case is for CI/build systems and self-hosted services


The goals of Gubernator, broadly, are as follows:

1. Facilitate easy-to-setup clusters for small-scale (i.e. < 100 nodes) uses
2. Jobs are lightweight by default, integrating with the node they run on
3. Be a good system citizen: don't require virtual machines images, admin permissions, or multi-gigabyte dedicated package managers
4. Facilitate cluster-agnostic jobs for portable-ish configurations
5. Act as primarily a back-end for specialized generators

The secondary goals (wishlist) are, in no particular order:

- Support single-node clusters where control and client are the same machine
- Support existing VM/container workloads
- Support less-trusted execution
- Declarative and purely functional cluster config (infrastructure as code)



### Easy-to-setup clusters

Creating a cluster should be trivial, with a simple creation flow with sensible
defaults (CLI not implemented and subject to change, this is the target level
of ease):


Client node:
```bash
apt install gubernator-client # or whatever, including local-user install
```


Gubernator (control machine):

```bash
apt install gubernator-control # or whatever, including local-user install

# add a client automatically based on existing ssh host
gubectl add-client --ssh user@client-node # signs cert and sends it over ssh(1)

# create client manually
gubectl add-client --hostname client-node --output client-node.crt
scp client-node.crt user@client-node:.config/gubernator/ # or use a USB drive or whatnot
```

Note that there is no expectation of Docker, and setup over existing local
networks should be automatic.


### Jobs are lightweight by default

Jobs are by default run as just normal processes/services. This is to make
adapting existing manually-run workloads as painless as possible, and to
minimize the resource requirements of a node. They should use packages
installed with the existing host package manager, run using the existing
service manager, and not rely on virtualization or container features unless
absolutely necessary for the workload.

The goal here is to mimic the workflow of ssh-ing to a node and running your
command or walking to your second computer and running the command in a new
terminal window.


### Be a good system citizen

Any computer you have an account for should be able to act as a node, which
means not causing problems for other users or requiring the entire system to be
dedicated to acting as one.


### Facilitate cluster-agnostic jobs

Consider the following example: a project has a CI pipeline definition in their
repository. You want to contribute, but naturally, their CI does not run
automatically for outsiders. After checking their CI pipeline definition (note
running untrusted code is *not* a goal here), you can run it on your own
cluster, with the machines you have and paste the results in your merge
request. Your cluster is different, but you have a variety of machines running
a variety of OS's so you still give the maintainer reason to believe your
change doesn't break anything.


### Act as a back-end for specialized generators

The canonical format for definitions is XML. XML is for precisely representing
complex data structures. Unless you are implementing a generator, you are not
expected to read nor write XML. To achieve cluster-agnostic jobs, requests must
encode a lot of information that would be unwieldy to write by hand.

Generators should read from a format appropriate to the task at hand. For
example a generator for a simple build system might just be a TOML file with a
target matrix, the dependencies needed for each target, and a sequence of
steps. The generator for a more complex and complete CI/CD system might instead
be written in [KDL](https://kdl.dev/) augmented with semantics.



## What does Gubernator not care about?

This list is subject to change

- Automatic provisioning: you should instead be giving your machines individual pithy names
- Multiple control nodes: I don't have enough scale to care about that
- Canonical templating engines: We already have enough semantics. You should be using a generator.
