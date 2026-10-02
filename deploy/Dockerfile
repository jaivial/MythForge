# MythForge API image: build with the pinned toolchain, run bare.
FROM rust:1.96-slim AS build
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/crates ./crates
RUN cargo build --release --bin forge-server

FROM debian:bookworm-slim AS run
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/forge-server /usr/local/bin/forge-server
ENV RUST_LOG=info,sqlx=warn
EXPOSE 8080
CMD ["forge-server"]
