# Bot

Richard is a friendly bot who loves FOSS.
It get into a room and speak whenever he wants.

# Features

Bot is modular and every module must be explicitely enabled.

Available modules:
- ping: responds to /ping commands with "pong"
- help: responds to /help command
- triggers: allow commands to be sent to all other modules
- endpoints: watch for Outscale API endpoints
- github_orgs: watch for all releases of all repositories of one or more github organisation
- github_repos: watch one or more specific githib repositories, trigger message on new release
- hello: send a random quote at a specific time interval
- ollama: interface with [ollama API](https://ollama.ai/), respond when no command is triggered
- feeds: watch for one or more RSS feeds, alert on new items
- roll: responds to /roll commands. e.g. /roll 1d20
- webpages: watch for one or more webpages. Alert when page content change.

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

