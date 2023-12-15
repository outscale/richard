# Bot

Richard is a friendly bot who loves FOSS.
It get into a room and speak whenever he wants.

# Features

- says hello, he is a gentleman
- speaks when he sees a new Outscale API version on production
- speaks when a region seems to be down or back on
- respond to few commands
- react when documentation page change

## Commands

Commands are read when Richard is notified in the configured room.
- `/ping`: respond `pong`
- `/status`: provide region status
- `/roll <dices>`: roll dices where `<dice>` is formated like `1d20` (1 dice of 20 faces)
- `/help`: show all available commands

# Build

1. Install [Rustlang](https://www.rust-lang.org/)
2. Run `cargo build --release`

If you need to have a static binary:
1. Install musl toolchain: `rustup target add x86_64-unknown-linux-musl`
2. Install `musl-gcc` (for Debian `apt install musl-tools`)
3. Build with `cargo build --target x86_64-unknown-linux-musl --release`

# Configure

Parameters are passed through environment variables. See [config.env.ori](./config.env.ori) example.
Use `--show-params` flag to print all needed var env per modules

As a facility, you can:
1. copy `config.env.ori` to `config.env`
2. edit `config.env`
3. load options by running `source config.env`

# Run

1. build or get pre-compiled binary
2. set configuration
3. `./richard`

