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
- `ping`: respond `pong`
- `status`: provide region status
- `emacs`: don't trigger this
- `roll <dices>`: roll dices where `<dice>` is formated like `1d20` (1 dice of 20 faces)
- `help`: show all available commands
- `describe <org_name> <repo_name> <version>`: descibe the content of a release 

# Build

1. Install [Rustlang](https://www.rust-lang.org/)
2. Run `cargo build --release`

If you need to have a static binary:
1. Install musl toolchain: `rustup target add x86_64-unknown-linux-musl`
2. Install `musl-gcc` (for Debian `apt install musl-tools`)
3. Build with `cargo build --target x86_64-unknown-linux-musl --release`

# Configure

Parameters are passed through environment variables.

- `WEBEX_TOKEN`: token provided by webex. See how to create a [controller bot](https://developer.webex.com/docs/bots).
- `WEBEX_ROOM_ID`: you can get room id by listing rooms (see below)
- `GITHUB_TOKEN`: Your Personal Access Token (PAT). See how to create a [PAT](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) with `public_repo,read:org`
- `RUST_LOG`: log level to use. example: `RUST_LOG=debug`. More details on [env_logger](https://docs.rs/env_logger/latest/env_logger/).

You can configure many regions (up to 100). Each region has a number starting from 0 to 99:
- `REGION_0_NAME`: friendly name for this region (e.g. "eu-west-2")
- `REGION_0_ENDPOINT`: whole endpoint URL (e.g. "https://api.eu-west-2.outscale.com/api/v1/")

You can configure many news feed (up to 100). Each feed has a number starting from 0 to 99:
- `FEED_0_NAME`: friendly name for this feed (e.g. "The Hacker News")
- `FEED_0_URL`: Atom, RSS or JSON feed URL (e.g. "https://feeds.feedburner.com/TheHackersNews")

As a facility, you can:
1. copy `config.env.ori` to `config.env`
2. edit `config.env`
3. load options by running `source config.env`

## Listing room ids and details

curl -H "Authorization: Bearer ${WEBEX_TOKEN}" "https://webexapis.com/v1/rooms" | jq

# Run

1. build or get pre-compiled binary
2. set configuration
3. `./richard`

# Test

1. set configuration
2. `cargo test`

# External resources

- [Webex API reference](https://developer.webex.com/docs/api/basics)
