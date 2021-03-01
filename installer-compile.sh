#!/bin/bash

sudo apt install git -y

echo "Installing docker"
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

sudo groupadd docker
sudo usermod -aG docker $USER

sudo apt install npm -y

echo "Installing rust components"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
sudo apt install cargo -y

source $HOME/.cargo/env

echo "Creating data directories"
mkdir -p ~/.kraken
cd ~/.kraken

echo "Fetching project files"
git clone https://github.com/ethanshry/Kraken.git .

cargo build --release --features vendored-openssl

echo "Setting up for auto-run on boot"

read -r -d '\n' fileout <<RUNNER
#!/bin/bash
cd ~/.kraken
LOCAL=\$(git rev-parse @)
REMOTE=\$(git rev-parse \${u})

# update in remote, re-fetch
if [ \$LOCAL != \$REMOTE]; then
    git pull
    curl -s https://api.github.com/repos/ethanshry/kraken/releases/latest \
        | grep "browser_download_url" \
        | cut -d : -f 2,3 \
        | tr -d \" \
        | wget -qi -
fi
cargo run --release --features vendored-openssl
RUNNER

echo "$fileout" >> /usr/local/bin/kraken_runner.sh
chmod 755 /usr/local/bin/kraken_runner.sh

read -r -d '\n' fileout <<CRON_CONFIG
@reboot /usr/local/bin/kraken_runner.sh
CRON_CONFIG

echo "$fileout" >> /var/spool/cron/crontabs/kraken
chmod 600 /var/spool/cron/crontabs/kraken
chown pi  /var/spool/cron/crontabs/kraken
chgrp crontab /var/spool/cron/crontabs/kraken

echo "Finished setting up kraken, reboot or execute '~/.kraken/kraken-rpi &' from ~/.kraken to run"