# ambit

[![Rust](https://github.com/plamorg/ambit/actions/workflows/rust.yml/badge.svg)](https://github.com/plamorg/ambit/actions/workflows/rust.yml)

Dotfile manager written in Rust.

`ambit` symbolically links files from a git repository to your system.
Files can be easily synced and are managed through a custom configuration file.

Features:

*   Custom configuration syntax
*   Git integration
*   System-based alternative files
    *   By operating system and hostname

## Getting Started

Use `ambit --help` for a list of possible commands.

### Initializing

`ambit` syncs dotfiles from a directory located at `${HOME}/.config/ambit/repo` by default.
This can be overridden by setting the `AMBIT_REPO_PATH` environment variable.

To initialize an empty dotfile repository:

    $ ambit init

To initialize from an existing dotfile repository:

    $ ambit clone <ORIGIN>

### Syncing

After a dotfile repository has been initialized,
simply run `ambit sync` to symlink files from the repository directory to the home directory as set by your configuration file.
If no configuration file is found, `ambit sync` will attempt to find a configuration file in `AMBIT_REPO_PATH`.

Use `ambit clean` to remove all symlinks created through the current configuration file.

### Git integration

Git commands run through `ambit` will be executed with `${HOME}/.config/ambit/repo` as the git directory.

For example, to show the working tree status of the repository:

    $ ambit git status

### Environment variables

Optionally, 3 environment variables can be used to set custom paths.
If a variable is not set, it will take up its default value as outlined:

| Environment Variable | Description                            | Default                              |
| -------------------- | -------------------------------------- | ------------------------------------ |
| AMBIT_HOME_PATH      | Starting path of symlink destinations. | Home directory                       |
| AMBIT_CONFIG_PATH    | Path to configuration file.            | `${HOME}/.config/ambit/config.ambit` |
| AMBIT_REPO_PATH      | Path to dotfile repository directory.  | `${HOME}/.config/ambit/repo`         |

## Configuration

The purpose of the configuration file is to set the paths of the symlinks.
A symlink is defined with two parts: an existing file relative to `AMBIT_REPO_PATH`, and its destination relative to the system's home directory.

### Configuration examples

#### Basic match

Symlink `${HOME}/host.txt -> ${AMBIT_REPO_PATH}/a/repo.txt`:

    a/repo.txt => host.txt;

#### Implicit match

If no `=>` operator is provided, it is assumed that the path given is both the `HOME` and `REPO` path.

Symlink `${HOME}/.config/ambit/config.ambit -> ${AMBIT_REPO_PATH}/.config/ambit/config.ambit`:

    .config/ambit/config.ambit;

#### Variant Expression

Symlink:

*   `${HOME}/.config/bat/bat.conf -> ${AMBIT_REPO_PATH}/.config/bat/bat.conf`
*   `${HOME}/.config/nvim/init.vim -> ${AMBIT_REPO_PATH}/.config/nvim/init.vim`

<!---->

    .config/[
        bat/bat.conf,
        nvim/init.vim
    ];

#### Match expressions

Conditionally symlink by os and host:

    {os(linux): .Xresources};

    {host(plamorg): .zshrc};

Combining conditionals:

    .config/bspwm/{os(linux):
        {
            host(foo): bspwmrc.foo,
            host(bar): bspwmrc.bar,
            default: bspwmrc,
        }
    } => .config/bspwm/bspwmrc;

## Installation

Build the `ambit` binary from source:

    $ git clone git@github.com:plamorg/ambit.git
    $ cd ambit
    $ cargo install --path .

## Development

Building:

    $ cargo build

Testing:

    $ cargo test

Formatting:

    $ cargo fmt

Linting:

    $ cargo check
    $ cargo clippy --all-targets --all-features
