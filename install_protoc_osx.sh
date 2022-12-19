#!/bin/bash
set -e
set -x

VERSION=21.9

ARCH=x86_64
# ARCH=aarch_64

OS=osx

# sudo apt-get -y install zip tree

mkdir temp
pushd temp
  curl -L https://github.com/protocolbuffers/protobuf/releases/download/v$VERSION/protoc-$VERSION-$OS-$ARCH.zip -o protoc.zip
  unzip -o protoc.zip -d protoc
  sudo mv protoc/bin/* /usr/local/bin/
  sudo mv protoc/include/* /usr/local/include/
popd
rm -rf temp

# tree /usr/local/include/google
