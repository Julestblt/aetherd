# syntax=docker/dockerfile:1

FROM rust:1.96-slim AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --all-features

FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /build/target/release/aetherd /usr/local/bin/aetherd

ENV AETHERD_HTTP__BIND=0.0.0.0:8080
EXPOSE 8080

USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/aetherd"]
