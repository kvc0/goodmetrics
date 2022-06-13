FROM rust:1.61 as build

RUN rustup component add rustfmt
RUN apt-get update
RUN apt-get install -y protobuf-compiler

WORKDIR /usr/src/goodmetrics
COPY . .
RUN cargo install --path .

# FROM debian:buster-slim
# # RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
# COPY --from=build /usr/local/cargo/bin/goodmetricsd /bin/goodmetricsd
ENTRYPOINT ["/usr/local/cargo/bin/goodmetricsd"]

