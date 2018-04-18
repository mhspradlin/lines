#!/bin/bash

PACKAGE_ROOT="$(dirname "$0")"/..

pushd $PACKAGE_ROOT

# Remove previous output files
rm debian/lines-agent_*

# Build the executable
cargo build --release

# Build the .deb
debuild

mv ../lines-agent_* debian/

debuild clean

popd
