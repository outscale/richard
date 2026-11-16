# syntax=docker/dockerfile:1.29@sha256:572cebbc2b15e6f84c077742952674889cb63cd5e9b80abdffcbe587b6f19183
FROM rust:1.96-trixie@sha256:1f0dbad1df66647807e6952d1db85d0b2bda7606cb2139d82517e4f009967376 AS builder
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM debian:trixie-slim@sha256:28de0877c2189802884ccd20f15ee41c203573bd87bb6b883f5f46362d24c5c2
COPY --from=builder /src/target/release/richard /usr/local/bin/richard
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENTRYPOINT ["/usr/local/bin/richard"]
