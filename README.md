# Kraken

[![Actions Status](https://github.com/ethanshry/Kraken/workflows/Rust/badge.svg)](https://github.com/ethanshry/Kraken/actions)

Kraken is designed to provide a platform of devices to which either persistant applications can be deployed to as a LAN-based development sandbox, or for one-off jobs to be processed either manually or automatically (i.e. running a script to scrape twitter data via the platform instead of your primary development machine, or automatically running unit tests on a repository whenever a github branch is updated).

The primary goal is to provide this platform in the way with the least friction possible, and at zero cost, making it possible to focus on the development of applications isolated from the concerns of hosting on AWS or other cloud services (i.e. dealing with EC2 management/configuration, the cost of servers, etc).

That being said, the number of possible hosting solutions for these types of solutions has grown exponentially in the last few years, and so more of the focus on this project recently has been on the learning of the rust ecosystem and building a more complete picture of what development in rust is like at this point in rust and the rust ecosystem's lifetime.

## Usage

Note that all Usage information is in flux at the current stage of the project, as the infrastructure is still being built out.

To setup Kraken on your LAN, you will (for now) need to clone this repository and build the dependencies manually. Follow the information in documentation/Development.md to install and build the application.

Once the application is running, the Kraken Platform Service(s) will be exposed on localhost including:

A RabbitMQ Host on port 5672
An API server including a graphql and graphiql instance on port 8080
Kraken as of now does not clean up Docker images, so you will need to kill and purge your images after running the service.

Currently, Kraken is in the process of enabling basic python and nodejs applicaton deployments. See [documentation/Deployment.md](documentation/Deployment.md) for more information.

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
