# Snowcap: Synthesizing Network-Wide Configuration updates

This project is a prototype implementation of the paper [Snowcap: Synthesizing Network-Wide Configuration Updates]() by Tibor Schneider, Rüdiger Birkner and Laurent Vanbever (Link will be created once the paper published in SIGCOMM '21).

**The [documentation](https://nsg-ethz.github.io/snowcap/snowcap/index.html) is hosted on GitHub Pages.**

## Abstract

Large-scale reconfiguration campaigns tend to be nerve-racking for network operators as they can lead to significant network downtimes, decreased performance, and policy violations. 
Unfortunately, existing reconfiguration frameworks often fall short in practice as they either only support a small set of reconfiguration scenarios or simply do not scale.

We address these problems with Snowcap, the first network reconfiguration framework which can synthesize configuration updates that comply with arbitrary hard and soft specifications, and involve arbitrary routing protocols. 
Our key contribution is an efficient search procedure which leverages counter-examples to efficiently navigate the space of configuration updates. 
Given a reconfiguration ordering which violates the desired specifications, our algorithm automatically identifies the problematic commands so that it can avoid this particular order in the next iteration.

We fully implemented Snowcap and extensively evaluated its scalability and effectiveness on real-world topologies and typical, large-scale reconfiguration scenarios. 
Even for large topologies, Snowcap finds a valid reconfiguration ordering with minimal side-effects (i.e., traffic shifts) within a few seconds at most.

## Building From Source

First, you need to install [`rustup`](https://www.rust-lang.org/tools/install). Then, you need to install the stable toolchain as follows:

```sh
rustup toolchain install stable
```

Optionally, you can install rustfmt (for formatting the code before contributing), rust-analyzer (For IDE integration using LSP) and flamegraph (For profiling the program):

```sh
rustup component add rust-src
rustup component add rustfmt
```

Then, build and test the project and the documentation with:

```sh
cargo build --release
cargo test --release
cargo doc --all-features
```

## Running `snowcap_main`

To run the `snowcap_main` binary, simply execute

```
cargo run -- --help
```

You can set the log level to something you wish, by setting the `RUST_LOG` environment variable. Possible values are `error`, `warn`, `info`, `debug` and `trace`:

```
RUST_LOG=info cargo run -- [ARGS]
```
