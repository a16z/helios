FROM docker.io/library/ubuntu:23.04

WORKDIR /opt/helios-test

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

COPY . /opt/helios-test

RUN apt-get -y update --fix-missing && \
  apt-get -y upgrade && \
  apt-get install -y build-essential pkg-config libfontconfig-dev && \
  apt-get install -y git curl wget jq lsof unzip vim && \
  # remove Cargo.lock otherwise get the following error:
  # error: failed to run custom build command for `yeslogic-fontconfig-sys v3.2.0`
  # reference: https://substrate.stackexchange.com/questions/7714/cannot-run-substrate-on-a-fresh-macbook-m2
  rm /opt/helios-test/Cargo.lock && \
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs/ | sh -s -- --default-toolchain nightly -y && \
  rustup default nightly-2023-07-31 && \
  rustup override set nightly-2023-07-31 && \
  rustup target add wasm32-unknown-unknown && \
  . /usr/local/cargo/env && \
  rustc --version && \
  rustup show && \
  cargo build

EXPOSE 8545

CMD tail -f /dev/null
