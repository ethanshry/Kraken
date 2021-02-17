# Development Setup

There are several tools you need to effectively develop this project.

- Make
- Rust (nightly)
- Docker
- Docker Compose
- Git
- npm/nodejs (to compile the UI)
- several other libraries I will need to find again oops

You will also need to add raspberry pi targets (and linker) if you want to cross-compile for rpi:

```bash
# For Pi 2/3/4
rustup target add armv7-unknown-linux-gnueabihf
# and install the linker
sudo apt install gcc-arm-linux-gnueabihf
```

## Docker configuration

When docker is initially installed on linux, it can only be managed as root. To work around this, follow the instructions [here](https://docs.docker.com/engine/install/linux-postinstall/). That being said, it can be done simply via the following commands:

```bash
sudo groupadd docker
sudo usermod -aG docker $USER
```

## .env

This project expects an environment file of the following format

```bash

RUST_LOG=[debug|info|error]
SHOULD_SCAN_NETWORK=[YES|NO]
SHOULD_CLONE_UI=[YES|NO]

```

| Variable            | Description                                                                                                                                                                                                                                                                                     |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| RUST_LOG            | Defines the log level for env_logger                                                                                                                                                                                                                                                            |
| SHOULD_SCAN_NETWORK | Defines whether or not the platform should look for an existing orchestrator on the network. A 'NO' value has the effect of running the platform as an orchestration node.                                                                                                                      |
| SHOULD_CLONE_UI     | This defines whether or not the orchestrator should clone the Kraken-UI or not. Cloning the UI provides a complete platform experience, however dramatically increased start-up time and is unnecessary for developing some portions of the backend or if you are already running the frontend. |

## Cross Compiling for RPi

So we need to compile openssl. Use instructions [here](http://web.archive.org/web/20160615142933/https://thekerneldiaries.com/2016/06/14/cross-compile-openssl-for-your-raspberry-pi/)

```bash
mkdir ~/pi
cd ~/pi
git clone https://github.com/raspberrypi/tools.git --depth=1 pitools
# this will create the tools directory at /path/to/basefolder/pitools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/bin
export CROSSCOMP_DIR=/home/ethanshry/pi/pitools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/bin

git clone https://github.com/openssl/openssl.git

export INSTALL_DIR=/home/ethanshry/pi/ssl-install
# then run configure
./Configure linux-generic32 shared \
--prefix=$INSTALL_DIR --openssldir=$INSTALL_DIR/openssl \
--cross-compile-prefix=$CROSSCOMP_DIR/arm-linux-gnueabihf-
```

make depend
make
make install

SEE THIS https://github.com/rust-embedded/cross/issues/229
https://stackoverflow.com/questions/37375712/cross-compile-rust-openssl-for-raspberry-pi-2

sudo apt install gcc-9-aarch64-linux-gnu
???
rustup target add aarch64-unknown-linux-gnu

https://github.com/jgallagher/amiquip/issues/20
https://github.com/briansmith/ring/issues/1193
