#! /bin/bash

# change directory to workspace
cd "$(dirname "$0")"

# generate the documentation
cargo doc --all-features || exit 1
RUSTDOCFLAGS="--html-in-header katex-header.html" cargo doc --no-deps --all-features || exit 1
echo '<meta http-equiv="refresh" content="0; url=snowcap/index.html">' > target/doc/index.html
