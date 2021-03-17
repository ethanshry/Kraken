# A LAN-Based Distributed Development Sandbox

By Ethan Shry

## Abstract

TODO: Blah blah blah

We first discuss the motivation and aims of the project.

We then discuss the capabilities and usage of the project

We then discuss the underlying technologies/systems which make the project possible

## Motivation

Over the past several years, the proliferation of cloud services like Amazon Web Services (AWS), Microsoft Azure, and Google Cloud Platform (GCP) have lowered the barrier to the development and deployment of web-based applications. Many companies have migrated their entire operations away from private data centers, and entirely rely on cloud offerings. Despite this, the cost of these solutions remains high for hobbyist developers. Below is a breakdown of the cost of hardware costs for dedicated, non-preemptible servers on the top three cloud platforms:

| Instance Name       | Specs           | Price (dollars/month) |
| ------------------- | --------------- | --------------------- |
| EC2 t2.micro (AWS)  | 1 CPUs, 1GB RAM | $8.47                 |
| A1 v2 (Azure)       | 1 CPUs, 2GB RAM | $26.28                |
| EC2 t2.large (AWS)  | 2 CPUs, 8GB RAM | $67.75                |
| A4 v2 (Azure)       | 4 CPUs, 8GB RAM | $116.07               |
| e2-standard-2 (GCP) | 2 CPUs, 8GB RAM | $48.91                |

While there are cheaper instances available if you opt for premptible services, or reserve hardware for an extended period of time, each platform has tens or hundreds of possible server configurations, which can make simply determining what you can get by with a challenge. Beyond the determination of appropriate server capacity, users of cloud platforms also need to develop the knowledge to make use of these servers- often requiring the management of Linux servers, firewall or security group settings, and the protection of cloud platform credentials. While these skills might be valuable in the workforce, often they add unnecesarry complexity to a project, especially during the early stages of development when the focus could be on the development of project features.

## Goals

This project aims to achieve three things:

- Reduce or eliminate the cost to deploy projects (at a LAN level)
- Reduce or eliminate the knowledge required to perform these deployments
- Be a testbed for the development of my knowlege in new areas of computer science

TODO: there is something to be said here about the types of deployments we are aiming to support (i.e. we are trying to support the hosting of websites or APIs locally so you can test them on different devices, or show them to friends, or power hobbyist projects, NOT support production scale deployments).

TODO: I'm not quite sure how to structure this section, something about why rust, something about pre-existing hardware, idk

### A Note on Scope of Work and Project Direction

While cost and complexity form the backbone of the motivation for the project, the primary motivation for its design and development revolve around the desire to develop knowledge in new areas of computer science. While it would have been trivial for me to use a group of technologies I was more comfortable with, or ignore reliability for what is fundamentally a development (and therefore not in need of stability) environment, I chose to focus on these areas of development. Though decisions made as a result of this third goal do not impact the first two in terms of the platform, it did significantly reduce the set of features I would have otherwise been able to build into the platform, and did inform some of the design decisions I made for the platform. I will endevor to make a note of these points shortcomings in the project as we cover its main systems.

## Platform Overview

The Kraken App Deployment Platform is a collection of devices all running the Kraken agent. The platform is made up of a single Orchestration node, and 0+ Worker nodes (although technically all nodes are both Orchestrators and Workers, see [Executors](###Executors) for more details). Users of the platform are able to access a web interface which allows them to monitor all devices (nodes) which are running the agent. From this interface, they can also request the deployment of an application to the platform via a Github Url. This will trigger the platform's orchestrator to validate the deployment, and select a worker to handle the deployment. Users can then use the interface to monitor their deployment- accessing information like deployment status, resource usage statistics, application logs, and request updates or destruction of a deployment.

TODO platform architecture diagram

### Platform Capabilities

The Platform supports the following deployment types:

- NodeJS Applications
- Python 3.6 Applications
- Static HTML Site Deployments

That being said, the platform is highly extensible. To add additional deployment types, all that is required is a dockerfile and a minor (potentially single line) code addition. Additionally, because the platform uses docker under the hood to manage deployments, you can also ship a custom dockerfile with your application, and run whatever deployment you want.

Due to the fact that it is designed to support primarilly API deployments, the platform will simply expose a single internal docker port externally to your machine. Ephemeral Tasks are technically supported, though their behavior has not been extensively validated. Other deployment types (databases, complex deployments utilizing docker networking features, etc) are not officially supported, though may be possible via the custom Dockerfile feature.

## Usage

This section will cover the important pieces needed to use the platform.

### Requirements

The platform relies on only a few external dependencies.

| Dependency     | Reason/Usage                                                                                                                                                                                                                                                                       |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Linux (Debian) | The platform was only tested on Debian (Ubuntu 20.04, 18.04, Raspbian Buster) distributions, and all installation/development/usage instructions are designed and tested under those assumptions. While other operating systems might function, they are not officially supported. |
| Git            | Used to download new versions of the platform, as well as deployment files                                                                                                                                                                                                         |
| Rustup/Cargo   | Used to compile the platform                                                                                                                                                                                                                                                       |
| Docker         | Used to manage deployments                                                                                                                                                                                                                                                         |
| NodeJS/npm     | Used to compile the platform interface                                                                                                                                                                                                                                             |
| systemctl      | Used to configure the auto-update and run-on-boot features of the platform, included by default with all tested Debian distributions                                                                                                                                               |

All of the required packages will be installed as part of the installation path you follow, see [Installation](###Installation) for more information.

### Installation

The goal of the installation process is to be as simple as possible for users. Therefore, it only takes the execution of a single script to get a system set up for use by the app deployment platform.

The platform can be installed to your device through one of three installation scripts:

| File                 | Purpose                                                                                                                                                                                      |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| installer-compile.sh | Installation for devices which can compile the project easily (i.e. laptop/desktop computers).                                                                                               |
| installer-dev.sh     | Installation and setup of dependencies for devices which will be developing the project (kraken peer dependencies). This will not set up the platform as a system service to be run on boot. |
| installer-pi.sh      | Installation for devices which cannot compile the project easily (i.e. raspberry pi)                                                                                                         |

Running an installer performs the following sequence of tasks:

```bash
# installation of relevant peer dependencies
# configuration of those peer dependencies (i.e. configuration of docker user groups)
# setup of kraken directory, and either compilation of the platform or downloading a compiled executable
# establishment of the kraken.service, which will run a script to auto-update and auto-run the platform on boot
```

We provide two different installation options (the `installer-compile` and `installer-pi`) since compiling a rust project (or this project in particualr) requires significant computing resources, as well as openssl, which does not come built in with Raspbian. As a result, prior to distributing a pre-compiled binary with openssl built in, compiling on a Raspberry Pi 3 B+ took upwards of an hour with active cooling, as well as additional configuration to get openssl installed. Now, Raspberry Pis simply need to download a ~20MB binary. See [Compilation](###Compilation) for more information about how Kraken achieves low-effort cross compilation.

The most detailed and up to date information about installation is available [here](./Installation.md).

### Application Onboarding

Getting your application onboarded to the platform requires only a few simple steps. First, you must make sure you have a running version of the platform on your LAN. You can either follow the [installation steps above](###Installation), or simply clone the project and run the platform temporarily.

Then you must configure your application to be run on the platform. To do this, you must simply create a `shipwreck.toml` file in the root of your repository, in the branch you want to be deployed to the platform. An example might look like the following:

```toml
[app]
name="scapenode"
version="1.0.0"
author="Ethan Shry"
endpoint="https://github.com/ethanshry/scapenode"

[config]
port=7211
lang="node"
run="npm start"
```

Components of the `app` section describe metadata about the application, while fields of the `config` section describe important configuration aspects of your deployment. More details about these fields are available on the `info` screen in the platform interface, and the config field format is implemented in the `crate::deployment::shipwreck` module.

From there, you simply need to access the `deployments` tab of the interface, and select the appropriate URL and branch for your deployment. This should enable the `Create Deployment` button, from which you can spin up your deployment.

![Deployment UI Example](./images/deployment_url_and_branch_ok.png)

## Platform/Program Systems

This section will cover the main systems involved in the platform. The figure below outlines the major responsibilities of a node on a platform.

![Node Components](./images/node_components_diagram.png)

As you can see, each node has two different `Executors` running at any given time. These are oulined more in the [Executors](###Executors) section, however an executor is a way to differentiate groupd of functionality. There are effectively two types of nodes on the platform- a single orchestrator, and many workers. Their responsibilities are covered in the [Workers](###Workers), [Orchestrators](###Orchestrators), and [Orchestration Rollover Candidates](###Orchestration-Rollover-Candidates) sections below.

The way the various pieces of the platform communicate with each other is outlined in [Communication](###Communication).

### Program Initialization + Main Program Loop

The platform initialization and execution is relatively straightforward, due to the use of executors. The setup process follows the following rough outline:

- Scan the LAN for an orchestration node by looking for an open REST endpoint at the Kraken REST API port number. If one exists, then start in worker mode. Otherwise start in orchestrator mode.
- Establish other relevant platform requirements (finding or creating a guid, figuring out the rabbitMQ address, determining a baseline rollover priority)
- Setup the orchestrator and worker executors

Then the execution loop is incredibly simple:

- Call `execute` on our `OrchestrationExecutor`, and handle an error if it occurs
- Call `execute` on our `WorkerExecutor`, and handle an error if it occurs

If we boil down this process to its simplest form, it looks something like this:

```rust

async fn main() -> Result<(), ()> {

    // Figure out if we are a worker or orchestrator by trying to find a platform to attach to
    let node_mode = match find_orchestrator_on_lan().await {
        Some(ip) => NodeMode::ORCHESTRATOR,
        None => NodeMode::WORKER
    };

    // other setup steps
    // ...

    // Setup our executors
    let mut orchestrator = OrchestrationExecutor::new();
    orchestrator.setup().await;
    let mut worker = WorkerExecutor::new();
    worker.setup().await;

    // The main execution loop
    loop {
        // Each node has an orchestration executor and a worker executor
        match orchestrator.execute(&mut node).await {
            Ok(_) => {}
            Err(faliure) => match faliure {
                // handle orchestrator faliures
                // this might involve orchestration rollover
                // ...
            },
        };
        match worker.execute(&mut node).await {
            Ok(_) => {}
            Err() => {
                // handle various forms of ExecutionFaliure
                // ...
            }
        };
        sleep(Duration::from_millis(500));
    }
}

```

You will note the use of a sleep at the end of this execution loop. This is simply to prevent the platform from performing hundreds of unnecesarry iterations if it has no work to do. While delta timing here might be preferred in a production environment and needed to gaurantee as quick as possible response times, in my experience this method has very little impact on platform responsiveness, and is marginally simpler, so I didn't worry about it.

### Executors

There are a variety of tasks a `Node` on the platform must perform. These broadly fall into two categories: features required of a `Worker`, and features required of an `Orchestrator`. While the specific responsibilities for each role will be outlined in the [Workers](###Workers), [Orchestrators](###Orchestrators), and [Orchestration Rollover Candidates](###Orchestration-Rollover-Candidates) sections, all of these roles follow the same `Executor` trait.

An executor is simply a struct which implements two methods: setup and execute. `setup` is called once to set up the node, and `execute` will then be called repeatedly until the executor crashes or the program terminates. Both of these methods also accept a `GenericNode`, which allows different executors to share common data, and return the same `SetupFaliure` and `ExecutionFaliure` responses across executors. This allows us to isolate different funcionality to specific executors, while having a unified way to interface with them. As a result of this design, it would be trivial to add another executor to extend the functionality of our `Node` without impacting existing executors, or running a `Node` which is only an `Orchestrator` or `Worker` without being both.

```rust
pub trait Executor {
    /// Is called once to set up this node
    async fn setup(&mut self, node: &mut GenericNode) -> Result<(), SetupFaliure>;
    /// Is called repeatedly after setup has terminated
    async fn execute(&mut self, node: &mut GenericNode) -> Result<(), ExecutionFaliure>;
}

pub struct GenericNode {
    pub broker: Option<RabbitBroker>,
    pub system_id: String,
    pub rabbit_addr: String,
    pub orchestrator_addr: String,
}
```

### Workers

A Worker, or more precisely a `WorkerExecutor`, handles all interfacing between the platform and the Docker Engine.

### Orchestrators

### Orchestration Rollover Candidates

### Communication

![Platform Communication](./images/platform_communication_diagram.png)

### In-Memory State Storage

### Rollover

### User Interface

### Compilation

### Limitations

TODO remove???

## Artifacts

| Artifact Name                 | Artifact Use                         | Link                                                                                                                     |
| ----------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| Kraken                        | Platform/Backend Code                | [https://github.com/ethanshry/Kraken](https://github.com/ethanshry/Kraken)                                               |
| Kraken-UI                     | UI/Frontend Code                     | [https://github.com/ethanshry/Kraken-UI](https://github.com/ethanshry/Kraken-UI)                                         |
| Kraken-Orchestrator-Discovery | Orchestrator IP Discovery Tool       | [https://github.com/ethanshry/Kraken-Orchestrator-Discovery](https://github.com/ethanshry/Kraken-Orchestrator-Discovery) |
| Kraken-Utils                  | Rust Crate to support Kraken Backend | [https://github.com/ethanshry/Kraken-Utils](https://github.com/ethanshry/Kraken-Utils)                                   |

## Acknowledgements

TODO flush this out:

Ryan Levick, Jon Gjenset, r/rust, Bill Siever
