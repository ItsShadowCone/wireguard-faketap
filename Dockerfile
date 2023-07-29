FROM rust:bookworm
WORKDIR /usr/src/myapp

ARG PRIVATE_KEY
ARG LISTEN_ADDR
ARG PERSISTENT_KEEPALIVE
ARG PEER_PUBLIC
ARG PEER_ENDPOINT

RUN apt-get -y update && apt-get -y install iproute2
COPY . .
RUN cargo install --path .
CMD ["./init.sh", "wireguard-faketap"]
