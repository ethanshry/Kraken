# Development Setup

There are several tools you need to effectively develop this project.

- Make
- Rust (nightly)
- Docker
- Docker Compose
- Git
- npm/nodejs (to compile the UI)
- several other libraries I will need to find again oops

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
