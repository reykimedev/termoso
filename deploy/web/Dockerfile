# Two images from one workspace:
#   docker build --target relay -t hatch-relay .
#   docker build --target web   -t hatch-web .   (needs: git submodule update --init)

FROM rust:1.97.0-slim-bookworm AS relay-build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
# The workspace also holds hatch-wasm, whose manifest names termoso-wasm in web/upstream.
COPY web/upstream/crates ./web/upstream/crates
COPY web/upstream/Cargo.toml web/upstream/Cargo.lock ./web/upstream/
RUN cargo build --release --locked -p hatch-relay

FROM gcr.io/distroless/cc-debian12:nonroot AS relay
COPY --from=relay-build /src/target/release/hatch-relay /usr/local/bin/hatch-relay
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/hatch-relay"]

# The browser's wasm module (hatch-wasm with termoso-wasm from web/upstream).
FROM rust:1.97.0-slim-bookworm AS wasm-build
RUN apt-get update && apt-get install -y --no-install-recommends git && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
RUN v="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)" \
 && cargo install wasm-bindgen-cli --locked --version "$v"
COPY crates ./crates
COPY web/upstream/crates ./web/upstream/crates
COPY web/upstream/Cargo.toml web/upstream/Cargo.lock ./web/upstream/
COPY web/build-wasm.sh ./web/
RUN ./web/build-wasm.sh /pkg

# The full image, not -slim: build.sh needs git to apply the patches.
FROM node:24-bookworm AS web-build
WORKDIR /src
COPY web ./web
COPY tools ./tools
COPY --from=wasm-build /pkg /pkg
ARG HATCH_VERSION=dev
ARG HATCH_COMMIT=dev
RUN HATCH_WASM_PKG=/pkg HATCH_VERSION="$HATCH_VERSION" HATCH_COMMIT="$HATCH_COMMIT" ./web/build.sh /src/dist

FROM nginxinc/nginx-unprivileged:1.29-alpine AS web
COPY --from=web-build /src/dist /usr/share/nginx/html
EXPOSE 8080
