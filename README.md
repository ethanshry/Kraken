# Kraken

[![Actions Status](https://github.com/ethanshry/Kraken/workflows/Rust/badge.svg)](https://github.com/ethanshry/Kraken/actions)

Kraken is a highly-scalable, distributed cloud deployment environment on any LAN. With minimal effort, a web application or ephemeral workflow can be configured to deploy to the Kraken platform, making it easier than ever to:

Test web applications on different physical hardware
Execute long-running tasks without bogging down your main computer
Persist application deployments past the runtime of your existing device
Eliminate costs associated with common cloud-based platforms for simple application testing

## Usage

Note that all Usage information is in flux at the current stage of the project, as the infrastructure is still being built out.

To setup Kraken on your LAN, you will need to clone this repository and build the dependencies manually. Follow the information in documentation/Development.md to install and build the application.

Once the application is running, the Kraken Platform Service(s) will be exposed on localhost including:

A RabbitMQ Host on port 5672
An API server including a graphql and graphiql instance on port 8080
Kraken as of now does not clean up Docker images, so you will need to kill and purge your images after running the service.

Currently, Kraken is in the process of enabling basic python applicaton deployments. Kraken will look for a kraken.toml in the root of your application's git repository with the following format:

```rust
[app]
name="[NAME]"
version="[SEMVER VERSION NUMBER]"
author="[AUTHOR]"
endpoint="[REPO URL]"

[config]
lang="[python36|undefined]" # The application lang indicates the base dockerfile
test="" # command to run tests, leave blank for no tests. Only passing tests will run application
run="python3 ./src/main.py" # command to run the application


[env] # any env vars associated with the deployment
test-var="test-var content"
```

## .env

This project expects an environment file of the following format

```bash

RUST_LOG=[debug|info|error]
NODE_MODe=[WORKER|ORCHESTRATOR]

```

## Development Setup

See /documentation/Development.md for information about developing this platform.

There are several tools you need to effectively develop this project.

- Make
- Rust (nightly)
- Docker

### Docker configuration

When docker is initiall installed on linux, it can only be managed as root. To work around this, follow the instructions [here](https://docs.docker.com/engine/install/linux-postinstall/). That being said, it can be done simply via the following commands:

```bash
sudo groupadd docker
sudo usermod -aG docker $USER
```

## Motivation and Project Information

There is a significant barrier to developing professional cloud development skills as a hobbyist developer. While it is now possible to provision cloud infrastructure through services like AWS and Microsoft Azure, there are two main barriers to entry for non-corporate developers:

Using cloud services comes with significant cost
Ways to manage development solutions are very confusing for junior developers, especially in the case of platform-specific technologies (i.e. AWS CodeDeploy or Azure VSTS)
There needs to be an easy way for junior developers to gain experience with professional development solutions without needing professional development experience- both to facilitate learning, and to encourage development best practices.

Most developers have some level of unused or under-used hardware lying around at home collecting dust which could be dedicated to cloud development if it were easy to provision and utilize.

Kraken exists to fill this need- with a minimal amount of setup, any existing hardware can be easily added to the Kraken app deployment platform, including low-cost hardware like a Raspberry Pi, all the way up to a powerful Laptop or Desktop computer.

Additionally, Kraken is my playgroud for learning about the Rust ecosystem, and developing strategies for handling distributed systems.

See /documentation/ProjectArchitecture.md for information about how the platform works.
