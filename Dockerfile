# syntax=docker/dockerfile:1.7
# Just Make It: GUI + API (jmi-server) y la CLI (jmi) en una imagen.
#
#   docker build -t just-make-it .
#   docker run --rm -p 8787:8787 -v "$PWD/out:/data/out" just-make-it          # GUI en http://localhost:8787
#   docker run --rm --user "$(id -u)" -v "$PWD:/work" -w /work just-make-it jmi render examples/titulo.json -o out/titulo.mp4

# ---------------------------------------------------------------- GUI
FROM node:22-slim AS gui
WORKDIR /gui
COPY gui/package.json gui/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY gui/ ./
RUN npm run build

# ---------------------------------------------------------------- motor (Rust + ffmpeg)
# Ubuntu para compilar y para la imagen final (misma libx264); el toolchain de Rust sale de la
# imagen oficial.
FROM rust:1-bookworm AS rust

FROM ubuntu:24.04 AS build
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential pkg-config ca-certificates git \
      clang libclang-dev nasm yasm ninja-build libx264-dev \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
COPY --from=rust /usr/local/rustup /usr/local/rustup
COPY --from=rust /usr/local/cargo /usr/local/cargo
RUN cargo --version
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# (la primera vez compila ffmpeg: ~10 min; los caches de cargo aceleran las siguientes)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p jmi-server -p jmi-cli \
    && mkdir -p /out && cp target/release/jmi-server target/release/jmi /out/

# ---------------------------------------------------------------- imagen final
FROM ubuntu:24.04
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends libx264-164 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 jmi \
    && mkdir -p /data/out && chown jmi /data/out
COPY --from=build /out/jmi-server /out/jmi /usr/local/bin/
COPY --from=gui /gui/dist /app/gui
COPY examples /app/examples
ENV JMI_ADDR=0.0.0.0:8787 JMI_GUI_DIR=/app/gui JMI_OUT_DIR=/data/out
USER jmi
WORKDIR /data
VOLUME /data/out
EXPOSE 8787
CMD ["jmi-server"]
