#!/bin/sh
# Build the site into build/.

rm -rf build
mkdir -p build
cp -r src/. build/ && echo "built"
