FROM rust:latest

WORKDIR /usr/src/myapp
COPY ./src ./src
COPY ./Cargo.toml ./Cargo.toml

RUN cargo install --path . ; rm -rf target
CMD ["mtx_server"]