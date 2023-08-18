FROM docker.io/library/ubuntu:23.04

ENV HOME /root
WORKDIR $HOME/helios

ENV RUSTUP_HOME=$HOME/rustup \
    CARGO_HOME=$HOME/cargo \
    PATH=$HOME/cargo/bin:$PATH

COPY . $HOME/helios

RUN apt-get -y update --fix-missing && \
  apt-get -y upgrade && \
  apt-get install -y build-essential pkg-config libfontconfig-dev && \
  apt-get install -y git curl wget jq lsof unzip vim && \
  # remove Cargo.lock and add `libfontconfig-dev` otherwise get the following error:
  # error: failed to run custom build command for `yeslogic-fontconfig-sys v3.2.0`
  test -f $HOME/helios/Cargo.lock && rm $HOME/helios/Cargo.lock && \
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs/ | sh -s -- --default-toolchain nightly -y && \
  rustup default nightly-2023-07-31 && \
  rustup override set nightly-2023-07-31 && \
  rustup target add wasm32-unknown-unknown && \
  . $HOME/cargo/env && \
  rustc --version && \
  rustup show && \
  cargo build

EXPOSE 8545 9001 9002

CMD tail -f /dev/null
