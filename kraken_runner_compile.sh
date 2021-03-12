#!/bin/bash
sleep 10
# TODO rename user to match you
cd /home/ethanshry/.kraken
LOCAL=$(git rev-parse @)
REMOTE=$(git rev-parse ${u})

# update in remote, re-fetch
if [ $LOCAL != $REMOTE]; then
    git pull
fi
cargo run --release --features vendored-openssl
