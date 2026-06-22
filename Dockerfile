# syntax=docker/dockerfile:1.24@sha256:87999aa3d42bdc6bea60565083ee17e86d1f3339802f543c0d03998580f9cb89
FROM rust:1.96-trixie AS builder
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM debian:trixie-slim
COPY --from=builder /src/target/release/richard /usr/local/bin/richard
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENTRYPOINT ["/usr/local/bin/richard"]
