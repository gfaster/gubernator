This document serves as both explanation and brainstorming.

# Machine

When a node connects to the control server, it sends a description of itself
and its capabilities.

TODO: if a node looses connection and reconnects, it should share information
about its running jobs

```xml
<machine>
    <!-- don't know if I want this as uname would make it redundant -->
    <arch>amd64</arch>

    <!-- /etc/os-release -->
    <os>
        <pretty_name>Debian GNU/Linux 13 (trixie)</pretty_name>
        <name>Debian GNU/Linux</name>
        <version_id>13</version_id>
        <version>13 (trixie)</version>
        <version_codename>trixie</version_codename>
        <debian_version_full>13.4</debian_version_full>
        <id>debian</id>
        <home_url>https://www.debian.org/</home_url>
        <support_url>https://www.debian.org/support</support_url>
        <bug_report_url>https://bugs.debian.org/</bug_report_url>
    </os>

    <!-- corresponding flags in GNU uname -->
    <uname>
        <name>Linux</name>
        <machine>x86_64</machine>
        <release>6.12.86+deb13-amd64</release>
        <version>#1 SMP PREEMPT_DYNAMIC Debian 6.12.86-1 (2026-05-08)</version>
        <operating_system>GNU/Linux</operating_system>
        <nodename>reimu</nodename>
    </uname>

    <desktop>
        <!-- TODO: figure out what should go here -->
    </desktop>

    <cpu>
        <!-- TODO: figure out what should go here -->
    </cpu>

    <memory>
        <!-- TODO: numa? -->
        <ram>15656MiB</ram>
        <swap>1024MiB</swap>
    </memory>

    <!-- note: using unicode homoglyphs in commands because xml comments -->
    <!-- run dpkg −−query -->
    <packages manager="dpkg">
        <pkg>
            <name>apparmor</name>
            <version>4.1.0-1</version>
        </pkg>
        <pkg>
            <name>apt</name>
            <version>3.0.3</version>
        </pkg>
        ...
    </packages>

    <!-- note: using unicode homoglyphs in commands because xml comments -->
    <!-- run dpkg −−query −−xml -->
    <packages manager="nix-env">
        <pkg>
            <name>neovim</name>
            <version>0.11.0</version>
        </pkg>
        ...
    </packages>

    <!-- flatpak list -->
    <packages manager="flatpak">
        <pkg>
            <name>Anki</name>
            <id>net.ankiweb.Anki</id>
            <version>25.09.02</version>
        </pkg>
        ...
    </packages>
</machine>
```
