FROM rust:1.81.0 as builder
WORKDIR /usr/src
RUN cargo new --bin changestream-to-redis
COPY Cargo.toml Cargo.lock /usr/src/changestream-to-redis/
WORKDIR /usr/src/changestream-to-redis
RUN cargo build --release
COPY src /usr/src/changestream-to-redis/src
RUN touch /usr/src/changestream-to-redis/src/main.rs
RUN cargo build --release

FROM debian:bookworm
COPY --from=builder /usr/src/changestream-to-redis/target/release/changestream-to-redis /changestream-to-redis
CMD ["/changestream-to-redis"]
