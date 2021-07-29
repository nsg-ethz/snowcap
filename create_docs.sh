#! /bin/bash

# change directory to workspace
cd "$(dirname "$0")"

# generate the documentation
cargo doc --all-features || exit 1
rm -rf \
   target/doc/snowcap \
   target/doc/snowcap_bencher \
   target/doc/snowcap_ltl_parser \
   target/doc/snowcap_main \
   target/doc/snowcap_runtime \
   target/doc/gns3
RUSTDOCFLAGS="--html-in-header katex-header.html" cargo doc --no-deps --all-features || exit 1

# generate the static webpage
cd website
hugo -D
cd ..

# copy over the static webpage
if [ -d "taget/doc/public" ]; then
    echo "removing the old static webpage"
    rm -rf target/doc/public
fi
echo "copying the new static webpage"
cp -r website/public target/doc/

# setup the index page
echo '<meta http-equiv="refresh" content="0; url=public/index.html">' > target/doc/index.html

# send the data to the server
rsync -avr -e ssh --delete target/doc/ web_snowcap@virt07.ethz.ch:public_html
