# Installation

## Setup

There are a few things you'll probably want to do prior to installation.

```bash

#!/bin/bash

sudo systemctl enable ssh
sudo systemctl start ssh

sudo apt update && sudo apt upgrade -y

```

## Installation

There are three scripts for installation.

| File                 | Purpose                                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------------------ |
| installer-compile.sh | Installation for devices which can compile the project easily.                                                     |
| installer-dev.sh     | Installation and setup of dependencies for devices which will be developing the project (kraken peer dependencies) |
| installer-pi.sh      | Installation for devices which cannot compile the project easily (i.e. a raspberry pi)                             |

## Raspberry Pi

Some things:

```bash

sudo systemctl enable ssh
sudo systemctl start ssh

sudo apt update && sudo apt upgrade -y

sudo apt install git -y

curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

sudo groupadd docker
sudo usermod -aG docker $USER

sudo apt install npm -y

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
sudo apt install cargo -y

source $HOME/.cargo/env

mkdir -p ~/.kraken
cd ~/.kraken

git clone https://github.com/ethanshry/Kraken.git .

# Kudos https://gist.github.com/steinwaywhw/a4cd19cda655b8249d908261a62687f8
curl -s https://api.github.com/repos/ethanshry/kraken/releases/latest \
    | grep "browser_download_url" \
    | cut -d : -f 2,3 \
    | tr -d \" \
    | wget -qi -



#cargo build --release --features vendored-openssl
```
