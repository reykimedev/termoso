# Labterm Web: two images from the repository root.
#   docker build -f deploy/web/Dockerfile --target relay -t labterm-relay .
#   docker build -f deploy/web/Dockerfile --target web   -t labterm-web .

FROM rust:1.97.0-slim-bookworm AS relay-build
WORKDIR /src
# The whole workspace, so cargo can resolve it; only labterm-relay is built.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/desktop/src-tauri ./apps/desktop/src-tauri
RUN cargo build --release --locked -p labterm-relay

FROM gcr.io/distroless/cc-debian12:nonroot AS relay
COPY --from=relay-build /src/target/release/labterm-relay /usr/local/bin/labterm-relay
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/labterm-relay"]

# The browser's wasm module (labterm-web-wasm with labterm-wasm linked in).
FROM rust:1.97.0-slim-bookworm AS wasm-build
RUN apt-get update && apt-get install -y --no-install-recommends clang=1:14.0-55.7~deb12u1 && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN v="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)" \
 && cargo install wasm-bindgen-cli --locked --version "$v"
COPY crates ./crates
COPY apps/desktop/src-tauri ./apps/desktop/src-tauri
COPY apps/web/build-wasm.sh ./apps/web/
RUN ./apps/web/build-wasm.sh /pkg

# The full image, not -slim: build.sh needs git to apply the patches.
FROM node:24-bookworm AS web-build
WORKDIR /src
COPY Cargo.toml ./
COPY crates/labterm-core/assets/autocomplete ./crates/labterm-core/assets/autocomplete
COPY apps/desktop ./apps/desktop
COPY apps/web ./apps/web
COPY --from=wasm-build /pkg /pkg
ARG HATCH_VERSION=dev
ARG HATCH_COMMIT=dev
RUN HATCH_WASM_PKG=/pkg HATCH_VERSION="$HATCH_VERSION" HATCH_COMMIT="$HATCH_COMMIT" ./apps/web/build.sh /src/dist

FROM nginxinc/nginx-unprivileged:1.29-alpine AS web
COPY deploy/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=web-build /src/dist /usr/share/nginx/html
EXPOSE 8080
