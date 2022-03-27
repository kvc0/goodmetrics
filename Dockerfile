FROM rust:1.59 as build

RUN rustup component add rustfmt

WORKDIR /usr/src/goodmetrics
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
# RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/local/cargo/bin/goodmetricsd /bin/goodmetricsd
CMD ["/bin/goodmetricsd"]
