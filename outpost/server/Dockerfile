# Outpost's game server, for a host that runs containers (Coolify, Fly.io...). Build it from the
# repo's root, which has the rules it shares with the browser (outpost/src/sim.rs) and the map:
#
#   docker build -f outpost/server/Dockerfile -t outpost-server .
#   docker run -p 8082:8082 outpost-server
#
# It speaks plain WebSocket on 8082; put it behind a proxy with TLS (Coolify's does it) so HTTPS
# pages can reach it at wss://, and set OUTPOST_SERVER_URL to that for the Laravel app.
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY outpost/src/sim.rs outpost/src/sim.rs
COPY outpost/server/Cargo.toml outpost/server/Cargo.lock outpost/server/
COPY outpost/server/src outpost/server/src
RUN cd outpost/server && cargo build --release --locked

FROM debian:bookworm-slim
COPY --from=build /src/outpost/server/target/release/outpost-server /usr/local/bin/outpost-server
COPY resources/outpost/map.json /srv/map.json
ENV OUTPOST_PORT=8082
EXPOSE 8082
CMD ["outpost-server", "/srv/map.json"]
