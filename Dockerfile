# syntax=docker/dockerfile:1

# Build a fully static binary against musl.
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev gcc
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY ui ./ui
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked && cp target/release/mailroom /mailroom

# The final image contains nothing but the binary.
FROM scratch
COPY --from=build /mailroom /mailroom
ENV MAILROOM_SMTP=0.0.0.0:1025 \
    MAILROOM_HTTP=0.0.0.0:8025
EXPOSE 1025 8025
USER 65534:65534
ENTRYPOINT ["/mailroom"]
