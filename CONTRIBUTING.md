# Install instructions

`cargo install`

# Testing

`cargo test`

# Debuging

You can get more details by setting environement variable `RUST_LOG`. Check [env_logger documentation](https://docs.rs/env_logger/0.9.3/env_logger/) for more details.

Examples:
```
export RUST_LOG="richard=trace"
export RUST_LOG="richard=info"
export RUST_LOG="richard=debug,richard::triggers=trace
```

# Questions / requests

Please open an issue

# Sending a Merge Request

If you plan to make some change in source code, consider making a pull request.
