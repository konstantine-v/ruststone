# syntax=docker/dockerfile:1

# ---- build ------------------------------------------------------------------
# Everything here (toolchain, registry, `target/`) is discarded: only the
# stripped binary is copied into the final image.
FROM rust:1-bookworm AS build
WORKDIR /src

# Strip symbols at link time; keeps the binary small without touching Cargo.toml.
ENV CARGO_PROFILE_RELEASE_STRIP=symbols

COPY Cargo.toml Cargo.lock ./
COPY src ./src
# The vendored selectors are compiled in with `include_str!`.
COPY assets ./assets

RUN cargo build --release --locked --bin ruststone-cli \
 && cp target/release/ruststone-cli /ruststone-cli \
 && rm -rf target "$CARGO_HOME/registry" "$CARGO_HOME/git"

# ---- runtime ----------------------------------------------------------------
# glibc + CA certificates (needed for HTTPS to the Lodestone), no shell or
# package manager. Same Debian release as the build stage.
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app

COPY --from=build /ruststone-cli ./ruststone-cli
COPY config ./config

ENV LOCO_ENV=production
EXPOSE 5150
ENTRYPOINT ["/app/ruststone-cli"]
CMD ["start"]
