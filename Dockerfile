FROM scratch
COPY target/x86_64-unknown-linux-musl/release/richard /
WORKDIR /
ENTRYPOINT ["/richard"]
