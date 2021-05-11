# syntax=docker/dockerfile:1
FROM ubuntu:18.04

# add the current project to the docker image
ADD . /snowcap/
VOLUME ["/snowcap/eval_sigcomm2021"]

# installation
RUN apt-get update
# install rustup and rustc
RUN apt-get install -qy curl ca-certificates file build-essential software-properties-common pkg-config libssl-dev libpcap-dev bc
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain stable -y
# install python and all the dependencies there
RUN apt-get install -qy python3.8 python3-pip
RUN python3.8 -m pip install --upgrade pip
RUN python3.8 -m pip install numpy pandas matplotlib
# install latex
RUN DEBIAN_FRONTEND=noninteractive apt-get install -qy texlive-latex-extra
# build Snowcap
RUN $HOME/.cargo/bin/cargo build --release --manifest-path snowcap/Cargo.toml

# start the main script
CMD ["/snowcap/eval_sigcomm2021/scripts/docker_entry.sh"]
