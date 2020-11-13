FROM clux/muslrust:1.47.0-stable as builder

COPY src/ /app/src
COPY Cargo.toml Cargo.lock /app/

RUN cd /app && \
    cargo build --release

FROM alpine:3.12.1

COPY --from=builder \
    /app/target/x86_64-unknown-linux-musl/release/do-updater \
    /usr/local/bin/do-updater

CMD ["do-updater"]
