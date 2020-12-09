# Development Setup

There are several tools you need to effectively develop this project.

- Make
- Rust (nightly)
- Docker
- Docker Compose

## Docker configuration

When docker is initially installed on linux, it can only be managed as root. To work around this, follow the instructions [here](https://docs.docker.com/engine/install/linux-postinstall/). That being said, it can be done simply via the following commands:

```bash
sudo groupadd docker
sudo usermod -aG docker $USER
```
