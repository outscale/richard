# syntax=docker/dockerfile:1.25@sha256:0adf442eae370b6087e08edc7c50b552d80ddf261576f4ebd6421006b2461f12
FROM rust:1.96-trixie AS builder
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM debian:trixie-slim
COPY --from=builder /src/target/release/richard /usr/local/bin/richard
ENTRYPOINT ["/usr/local/bin/richard"]
