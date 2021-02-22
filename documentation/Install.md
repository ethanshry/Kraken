# Installation

## Raspberry Pi

Some things:

```bash

sudo systemctl enable ssh
sudo systemctl start ssh

sudo apt install git -y

curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

sudo groupadd docker
sudo usermod -aG docker $USER

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
sudo apt install cargo -y

source $HOME/.cargo/env

mkdir -p ~/.kraken
cd ~/.kraken

git clone https://github.com/ethanshry/Kraken.git .
cargo build --release --features vendored-openssl
```
