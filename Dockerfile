FROM alpine
COPY target/x86_64-unknown-linux-musl/release/richard /usr/local/bin/richard
CMD /usr/local/bin/richard