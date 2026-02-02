
# mc2

Provides user configured development environments.

An continuation of [ooxi/mini-cross](https://github.com/ooxi/mini-cross).

## Configuration

### Config Locations
Config files name be placed into the one of the following locations relative to the work directory:
- \<machine\>.yaml
- .mc/\<machine\>.yaml
- .mc/\<machine\>/\<machine\>.yaml

> If machine name is not provided, machine name will default to 'mc'.

Additional lookup paths for machines can be defined in `.mc2aliases.yaml`, wich contains a map from 
machine name to relative path of alias file.

```yaml
my-custom-machine: ../custom-toolchains/brainfuck
```

### Environment Config

```yaml
---
base: ubuntu:22.04	# ①
install:
 - nodejs		# ②
 - npm
publish: # TODO
 - 8080:80		# ③
 - 8443:443
---
#!/bin/bash

cargo install mc2	# ④
```

The configuration file contains two sections: first a YAML frontmatter section
followed by an optional shell script. Splitting the configuration into a
declarative and an imperative section enables describing common operations with
minimal boilerplate while still allowing arbitary actions.

1. `base` describes the [Docker image][base-docker-image] to be used as starting
   point for further setup.
2. `install` contains a list of packages to be installed from the distribution's
   package manager
3. `publish` contains a list of `<host port>:<container port>` declarations
   describing [port forwarding][docker-publish] from host to container
4. A shell script containing arbritrary commands to be executed while creating
   the container's image

Since mini-cross needs to know how to install packages on a certain
distribution, not all Docker images are supported as base images. Current
support includes:

-Arch Linux (Untested)
- Debian  (Untested)
- Fedora
- OpenSuse (Untested)
- Ubuntu  (Untested)


[base-docker-image]: https://docs.docker.com/engine/reference/builder/#from
[docker-publish]: https://docs.docker.com/engine/reference/run/#expose-incoming-ports


## CLI

There two ways of invoking mini-cross

1. The command invocation
2. The shell invocation

While technically similar, they provide for different use cases. The first
allows to run individual commands inside the development environment while
remaining attached to the host shell. The second changes the point of view to
the inside of the development environment so that multiple commands can be
executed while attached to the same container.

Therefore the command invocation is more suitable for scripted usage while the
second is crafted toward comfort for interactive use.



### Command invocation

    mc2 --help


When using mini-cross with command invocation, a *machine* has always to be
specified. The *machine* determines where to look for the mini-cross
configuration and allows for multiple configurations in the same project. The
special machine `_` is the default machine (most useful for shell invocation
though).

This command will start the referenced machine and execute the command using
the default docker entry point (most likely a [bash][1] shell).



### Shell invocation

    mc2 [<machine>]

Invoking mini-cross without additional commands is referred to as shell
invocation because the development environment will stay attached to the current
shell. Multiple commands may now be executed inside the same container, which
can be quit using the exit command (assuming the container uses a shell like
[bash][1] as docker entry point).

Since the shell invocation does not use arguments, the machine name can be
omited and `_` (the default machine) will be assumed.





[1]: https://www.gnu.org/software/bash/
