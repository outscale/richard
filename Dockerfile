# syntax=docker/dockerfile:1.7
FROM rust:1.96-trixie AS builder
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM debian:trixie-slim
COPY --from=builder /src/target/release/richard /usr/local/bin/richard
ENTRYPOINT ["/usr/local/bin/richard"]
