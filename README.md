# ambit

Dotfile manager written in Rust.

(Planned) features:

*   Git integration
*   System-based alternative files
*   Custom configuration syntax

WARNING! Note that `ambit` is currently under development, basic features may not work.

## Getting Started

Use `ambit --help` for a list of possible commands.

### Initializing

`ambit` syncs dotfiles from a directory located at `${HOME}/.config/ambit/repo`.

To initialize an empty dotfile repository:

    $ ambit init

To initialize from an existing dotfile repository:

    $ ambit clone <ORIGIN>

### Git integration

Git commands run through `ambit` will be executed with `${HOME}/.config/ambit/repo` as the git directory.

For example, to show the working tree status of the repository:

    $ ambit git status

## Configuration

`ambit` will search for a configuration file located at `${HOME}/.config/ambit/config`.

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
