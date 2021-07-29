+++
title = "Setup and Usage"
description = "Instructions on how to install and use Snowcap"
date = "2021-05-19"
author = "Tibor Schneider"
sec = 4
+++

_Snowcap_ is written in Rust, and can be used as an executable, or as a library.
To install the rust toolchain, follow the instructions on installing [rustup](https://rustup.rs/).
Then, clone and build the project:

```sh
git clone git@github.com:nsg-ethz/snowcap.git
cd snowcap
cargo build --release
```

Then, you can run snowcap with:

```sh
cargo run --release -- --help
```
