#! /bin/bash

# change directory to workspace
cd "$(dirname "$0")"

# check if target/doc is already a git repo.
if [ ! -d "target/doc/.git" ]; then
    # remove the doc folder (only if it exists)
    if [ -d "target/doc" ]; then
        rm -rf target/doc
    fi

    # go to the doc folder
    mkdir -p target/doc

    # pull the correct repository
    git clone -b gh-pages git@github.com:nsg-ethz/snowcap.git target/doc/
fi

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
echo '<meta http-equiv="refresh" content="0; url=https://nsg-ethz.github.io/snowcap/public/index.html">' > target/doc/index.html

# go to the docs folder
cd target/doc

# check if there is something to commit
UPDATED_FILES=$(git ls-files -o -m -d)
if [ -z $UPDATED_FILES ]; then
    echo "Nothing has changed! Nothing to push"
    cd ../..
    exit 0
else
    echo "Files changed:"
    echo "$UPDATED_FILES"
fi
echo ""

# read a git commit message
read -p "Enter a git message for github pages: " COMMIT_MESSAGE || exit 1

# Create the commit
git add .
git commit -m "${COMMIT_MESSAGE}"
git push origin gh-pages

# go back to the root
cd ../..
