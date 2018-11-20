#!/usr/bin/env bash

set -eu

BREW_MUSL_BINARY="x86_64-linux-musl-gcc"
LINUX_MUSL_BINARY="musl-gcc"

VARS_FILE="../mailgun-contact-form-deploy/vars.yml"

MUSL_BINARY=""

if [[ ! -z "$(which ${BREW_MUSL_BINARY})" ]]
then
    MUSL_BINARY="$BREW_MUSL_BINARY"
elif [[ ! -z "$(which ${LINUX_MUSL_BINARY})" ]]
then
    MUSL_BINARY="$LINUX_MUSL_BINARY"
else
    echo "Cannot find a musl-gcc binary - please run either 'brew install filosottile/musl-cross/musl-cross' on MacOS, or 'apt-get install musl-tools' on Ubuntu"
    exit 1
fi

if [[ ! -f ${VARS_FILE} ]]
then
    echo "Cannot find the file $VARS_FILE - please create a file with the vars to use when interpolating the CF manifest"
    exit 1
fi

CC_x86_64_unknown_linux_musl="$MUSL_BINARY" cargo build --release --target=x86_64-unknown-linux-musl

pushd target/x86_64-unknown-linux-musl/release && zip mailgun-contact-form mailgun-contact-form ; popd

cf push --vars-file ${VARS_FILE}