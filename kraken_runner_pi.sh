#!/bin/bash
sleep 10
cd /home/pi/.kraken
LOCAL=$(git rev-parse @)
REMOTE=$(git rev-parse ${u})

# update in remote, re-fetch
if [ $LOCAL != $REMOTE]; then
    git pull
    rm kraken-rpi
    curl -s https://api.github.com/repos/ethanshry/kraken/releases/latest \
        | grep "browser_download_url" \
        | cut -d : -f 2,3 \
        | tr -d \" \
        | wget -qi -
    chmod 777 kraken-rpi
fi
./kraken-rpi
