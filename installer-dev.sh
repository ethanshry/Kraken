#!/bin/bash
echo "Installing Kraken peer dependencies"

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

echo "Finished setting up Kraken dependencies for development"