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
sudo cp ./kraken_runner_compile.sh /usr/local/bin/kraken_runner.sh
chmod 755 /usr/local/bin/kraken_runner.sh

sudo cp ./kraken.service /etc/systemd/system/kraken.service
sudo chmod 644 /etc/systemd/system/kraken.service
sudo chown root  /etc/systemd/system/kraken.service

sudo systemctl daemon-reload
sudo systemctl enable kraken.service

echo "Finished setting up kraken, reboot or execute 'sudo systemctl start kraken.service' to run"