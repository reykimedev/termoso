# Two images from one workspace:
#   docker build --target relay -t hatch-relay .
#   docker build --target web   -t hatch-web .   (needs: git submodule update --init)

FROM rust:1.97.0-slim-bookworm AS relay-build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --release --locked -p hatch-relay

FROM gcr.io/distroless/cc-debian12:nonroot AS relay
COPY --from=relay-build /src/target/release/hatch-relay /usr/local/bin/hatch-relay
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/hatch-relay"]

# The full image, not -slim: build.sh needs git to apply the patches.
FROM node:24-bookworm AS web-build
WORKDIR /src
COPY web ./web
RUN ./web/build.sh /src/dist

FROM nginxinc/nginx-unprivileged:1.29-alpine AS web
COPY --from=web-build /src/dist /usr/share/nginx/html
EXPOSE 8080
