#!/bin/bash

echo "Installing rust components"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
sudo apt install cargo -y

echo "Creating data directories"
mkdir -p ~/.kraken
cd ~/.kraken

echo "Fetching project files"
git clone https://github.com/ethanshry/Kraken.git .

cargo build --release

echo "Setting up for auto-run on boot"

read -r -d '\n' fileout <<RUNNER
#!/bin/bash
cd ~/.kraken
LOCAL=\$(git rev-parse @)
REMOTE=\$(git rev-parse \${u})

# update in remote, re-fetch
if [ \$LOCAL != \$REMOTE]; then
    git pull
    cargo build --release
fi
cargo run &
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

echo "Finished setting up kraken, reboot or execute 'cargo run &' from ~/.kraken to run"