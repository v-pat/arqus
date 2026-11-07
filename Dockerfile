# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
ARG EXTRA_PACKAGES=""
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates libssl3 git curl build-essential pkg-config python3 ripgrep jq less procps ${EXTRA_PACKAGES} \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/pirs /usr/local/bin/pirs
RUN useradd -m -u 1000 -s /bin/bash agent
USER agent
ENV HOME=/home/agent PIRS_HOME=/home/agent/.pirs TERM=xterm-256color
WORKDIR /work
ENTRYPOINT ["pirs"]
