# Kraken

[![Actions Status](https://github.com/ethanshry/Kraken/workflows/Rust/badge.svg)](https://github.com/ethanshry/Kraken/actions)

LAN-based cloud deployment environment

## Development Setup

There are several tools you need to effectively develop this project.

- Make
- Rust (nightly)
- Docker

## Docker configuration

When docker is initiall installed on linux, it can only be managed as root. To work around this, follow the instructions [here](https://docs.docker.com/engine/install/linux-postinstall/). That being said, it can be done simply via the following commands:

```bash
sudo groupadd docker
sudo usermod -aG docker $USER
```

## .env

This project expects an environment file of the following format

```bash

RUST_LOG=[debug|info|error]
NODE_MODe=[WORKER|ORCHESTRATOR]

```
