# syntax=docker/dockerfile:1
# `codecast video` with everything it needs: headless Chromium (the build that draws frames on
# demand), ffmpeg and fonts. Built from this repository (scripts/iwr video builds and runs it):
#   docker build -t codecast-video -f docker/video/Dockerfile .
#   docker run --rm -v "$PWD:/project" codecast-video video /project/codecast

# ---- build: the web UI (dx) and the CLI that embeds it
FROM rust:1.95-trixie AS build
RUN rustup target add wasm32-unknown-unknown \
 && curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
 && cargo binstall -y --locked dioxus-cli@0.7.10
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cd crates/iwr-ui && dx build --release && cd ../.. \
 && cargo build --release -p iwr \
 && cp target/release/codecast /usr/local/bin/codecast

# ---- run
FROM debian:trixie-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends chromium-headless-shell ffmpeg ca-certificates \
      fonts-noto-core fonts-noto-mono fonts-noto-cjk fonts-noto-color-emoji \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/local/bin/codecast /usr/local/bin/codecast
# a non-root user (`docker run --user`) still needs a writable home for Chromium's profile
ENV HOME=/tmp
ENTRYPOINT ["codecast"]
CMD ["--help"]
