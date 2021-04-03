# A LAN-Based Distributed Development Sandbox

By Ethan Shry

## Abstract

While modern cloud platforms have significantly lowered the barrier to entry for public deployment of web applications, they can still be incredibly costly and add significant complexity to the development process, especially for hobbyist developers or in the beginning stages of a project. In this paper I propose a LAN-based application deployment environment which will allow developers to quickly and easily test web applications on devices on their LAN, without the need to pay for or manage cloud resources.

In [Section I. Background](##I.-Background), I will discuss background information for this project, including the project's primary aims. In [Section II. Platform Overview](##II.-Platform-Overview), I will provide a broad overview of the platform and its capabilities. In [Section III. Usage](##III.-Usage), I will discuss the installation and onboarding process for using the platform to deploy applications to a LAN. Finally, [Section IV. Platform Systems](##IV.-Platform-Systems) discusses in depth the major technical systems which allow the platform to achieve the features described in Section II.

## I. Background

### Motivation

Over the past several years, the proliferation of cloud services like Amazon Web Services (AWS), Microsoft Azure, and Google Cloud Platform (GCP) have lowered the barrier to the deployment of publicly accessible web applications. Most companies have at least some cloud presence, and some are transitioning entirely away from on-premise datacenters entirely \[1\]. Despite this, the cost of these solutions remains high for hobbyist developers. Below is a breakdown of the cost of hardware costs for dedicated, non-preemptible linux virtual machines on the top three cloud platforms as of March 2021:

| Instance Name | Platform | Specs           | Price (dollars/month) \[2\]\[3\]\[4\] |
| ------------- | -------- | --------------- | ------------------------------------- |
| EC2 t2.micro  | AWS      | 1 CPUs, 1GB RAM | $8.47                                 |
| A1 v2         | Azure    | 1 CPUs, 2GB RAM | $26.28                                |
| EC2 t2.large  | AWS      | 2 CPUs, 8GB RAM | $67.75                                |
| A4 v2         | Azure    | 4 CPUs, 8GB RAM | $116.07                               |
| e2-standard-2 | GCP      | 2 CPUs, 8GB RAM | $48.91                                |

While there are cheaper instances available if you opt for preemptible services, or reserve hardware for an extended period of time, each platform has tens or hundreds of possible server configurations, which can make simply determining the correct offering for your use case a hassle. Beyond the determination of appropriate server capacity, users of cloud platforms also need to dedicate time to develop their knowledge to make use of these servers- often requiring the management of Linux servers, firewalls, security group settings, remote code deployment pipelines, and the management of cloud platform credentials, to name a few things. While these are undoubtedly valuable in a production application, they can add unnecessary complexity to any project, especially during the early stages of development when the focus could be on the development of project features.

### Goals

This project aims to achieve two things:

- Reduce or eliminate the cost to deploy web applications for development or local use
- Reduce or eliminate the time and knowledge required to deploy the aforementioned applications

It is important to understand the types of deployments this project is aiming to support- we are not trying to replace the cloud for production applications in use by thousands of users daily around the world, or even to host a personal website visited by a few hundred people a month. Rather, this project is aiming to allow developers to host local APIs to power hobbyist IoT projects, or provide an easy means for a developer to test a project they are developing actually builds in isolation from the other dependencies on their system. This could be used to facilitate smooth adoption of new collaborators to a project, or in preparation for deployment to a cloud environment. In either of these cases, the actual migration of a project to a broader team or to the cloud is out of scope of this project, though a migration guide could be a worthwhile future addition.

To this end, the platform needs to be as easy to use as possible and able to easily be installed and run on whatever hardware users have available. The vision is to have a platform versatile enough to be installed and running across a modern desktop, old laptop, and Raspberry Pi all at once, and be flexible enough to handle any number of these devices being added or removed from the network. If the platform is installed on a laptop and the laptop is taken out of the house, then it should be resilient enough to maintain its deployments, and able to easily reconnect the device when it returns to the network.

### Related Work

The niche this platform satisfies has no direct replacements, due to its unique positioning as a no-cost and easily managed solution.

While there are plenty of cloud application deployment platforms, (AWS Elastic Beanstalk, Azure's App Service Deployment Center, etc.) that are flexible enough to support API applications, the knowledge and cost barrier to using these services is the same as broader cloud solutions. There are also options for directly managing API services (CloudFlare Service Workers, AWS's API Gateway, etc.) are fairly restrictive in their capabilities, and even more challenging to manage. You could look to more flexible and capable cloud VM providers, however the cost for these is significant (as described above) and there is additional overhead in the management and maintenance of these machines.

While there are locally-hosted options (namely Docker for local deployments, and Kubernetes for wholistic application deployment and management), Docker on its own is not sufficiently flexible to satisfy our distributed and easily-manageable requirements, as it is isolated to a single device, and while it is easier to manage than manual dependency management, still has significant management overhead. Kubernetes, on the other hand, is infamous for developer's lack of ability to understand what it is or how to use it- even among companies actively interested in utilizing the technology over 40% cite the complexity and lack of developer expertise as key challenges, which speaks to the knowledge overhead involved in effectively understanding and managing a Kubernetes cluster \[5\].

### A Note on Scope of Work and Project Direction

While cost and complexity form the backbone of the motivation for the project, the primary motivation for its design and development revolve around the desire to develop knowledge in new areas of computer science. While it would have been (relatively) trivial to implement this system in a technology stack in which I have development experience, or ignore reliability for what is fundamentally a development (and therefore not truly in need of stability) environment, I chose to focus on these areas of development as a matter of personal growth. Though decisions made as a result of this motivation did not impact my ability to accomplish the platform's primary goals, it did reduce the set of features I initially wanted to build into the platform, and did inform some of the design decisions I made in its development. I will endeavor to make a note of these shortcomings and alterations to project direction as I cover its main systems.

## II. Platform Overview

The Kraken App Deployment Platform is a collection of devices running the Kraken service. The platform is made up of a single Orchestration node, and 0+ Worker nodes (although technically all nodes are both Orchestrators and Workers, see [Executors](###Executors) for more details). Users of the platform are able to access a web interface which allows them to monitor all devices (nodes) which are running the service. From this interface, they can also request the deployment of an application to the platform via a Github URL. This will trigger the platform's orchestrator to validate the deployment and select a worker to handle the deployment via Docker. Users can then use the interface to monitor their deployment- accessing information like deployment status, resource usage statistics, application logs, and request updates or destruction of a deployment.

| Technology       | Purpose                                                                          |
| ---------------- | -------------------------------------------------------------------------------- |
| Rust             | Language used for the backend development of the platform                        |
| Tokio            | Library used to provide the backend with an asynchronous runtime / green threads |
| Rocket           | Library used for the backend REST API                                            |
| GraphQL          | Used to communicate between the UI and the Backend                               |
| RabbitMQ         | Used to communicate between nodes                                                |
| Docker           | Used to manage Deployments/Containers                                            |
| Systemd          | Used to manage the running and updating of the service after installation        |
| React/Typescript | Used for UI development                                                          |

### Platform Capabilities

The platform has been tested with support for the following deployment types:

- NodeJS 12.x Compatible Applications
- Python 3.6 Compatible Applications
- Static HTML Sites
- Custom Dockerfile Deployments

The platform is highly extensible. To add additional deployment types, all that is required is a dockerfile and a minor (potentially single line) code addition. Additionally, because the platform uses docker under the hood to manage deployments, you can ship a custom dockerfile with your application, and run whatever deployment type you want.

When a deployment is made, the UI provides insight into the state of that deployment. You can track its progress through the deployment process, as well as access to container usage information like CPU and RAM usage. Additionally the UI provides easy access to application logs for complete application monitoring.

Deployments can be made from any branch in a github repository, so long as that branch has the required configuration file. Once a deployment has been made, it is a single button click to re-deploy the application from the same git branch, or to shut down the deployment.

Due to the fact that it is designed to support primarily API deployments, the platform will simply expose a single internal docker port externally to your machine/LAN. Ephemeral Tasks are technically supported, though their behavior has not been extensively validated. Other deployment types (databases, complex deployments utilizing docker networking features, etc) are not officially supported, though may be possible via the custom Dockerfile feature.

## III. Usage

### Requirements

The platform relies on a few external dependencies.

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

The goal of the installation process is to be as simple as possible for users. Therefore, it only takes the execution of a single script to get a system set up for use by the Kraken service.

The platform can be installed to your device through one of three installation scripts:

| File                 | Purpose                                                                                                                                                                                      |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| installer-compile.sh | Installation for devices which can compile the project easily (i.e. laptop/desktop computers).                                                                                               |
| installer-dev.sh     | Installation and setup of dependencies for devices which will be developing the project (kraken peer dependencies). This will not set up the platform as a system service to be run on boot. |
| installer-pi.sh      | Installation for devices which cannot compile the project easily (i.e. raspberry pi)                                                                                                         |

Running an installer performs the following sequence of tasks:

- installation of relevant peer dependencies
- configuration of those peer dependencies (i.e. configuration of docker user groups)
- setup of kraken directory, and either compilation of the platform or downloading a compiled executable
- establishment of the kraken.service, which will run a script to auto-update and auto-run the platform on boot

There are two different installation options (the `installer-compile` and `installer-pi`) since compiling a rust project (or this project in particular) requires significant computing resources, as well as openssl, which does not come built in with Raspbian. As a result, prior to distributing a pre-compiled binary with openssl built in, compiling on a Raspberry Pi 3 B+ took upwards of an hour with active cooling, as well as additional configuration to get openssl installed. Now, Raspberry Pis simply need to download a ~20MB binary (this has not been optimized for size, Rust is notorious for having large binary sizes prior to optimization due to static linking of all dependencies and static dispatch for generics). See [Compilation](###Compilation) for more information about how Kraken achieves low-effort cross compilation.

The most detailed and up to date information about installation is available [here](./Installation.md).

### Binary Compilation

Due to the low resources of a Raspberry Pi, compiling this project natively can take hours. For this reason, the project is configured to support cross-compilation for Raspberry Pi. By making use of Rust's tooling, this process is actually fairly straightforward.

First, you must define the target in your rust build tools (cargo) configuration file:

```toml
[build]

# Pi 2/3/4
[target.armv7-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"
```

Then you install the relevant linker to your system you are compiling from:

```bash
sudo apt install arm-linux-gnueabihf-gcc
```

To compile for RPi, you must also bundle in openssl. Luckily with Cargo's `features` feature, this becomes trivial. We first add `openssl` as an optional dependency to our `Config.toml` (which is the Rust `package.json` or `requirements.txt` equivalent):

```toml
openssl = { version = '0.10', optional = true }
```

and then add the openssl feature to our `Config.toml`

```toml
[features]
vendored-openssl = ["openssl/vendored"]
```

Then simply compile the project as follows:

```bash
cargo build --release --target armv7-unknown-linux-gnueabihf --features vendored-openssl
# or for this project, just use the shortcut defined in the Makefile
make build-pi
```

This will generate an executable, which can be uploaded to Github as a new release. The most recent release will be downloaded by `installer-pi.sh` as part of the installation process, or on each boot if a newer binary is available.

### Application Onboarding

The application onboarding process is as follows:

- Install/Run the platform
- Create a `shipwreck.toml` file in your project's root specifying configuration information
-

Onboarding an application to the platform requires only a few simple steps. First, you must have a running version of the platform on your LAN. You can either follow the [installation steps above](###Installation) to install the platform as a service, or simply clone the project and run the dev installation script, and compile and build locally.

Suppose we have a basic NodeJS express server, with a file structure and file contents:

```bash
# Directory Structure
.
├── package.json
└── src
    └── index.js
```

```js
// index.js
const express = require("express");
let app = express();

app.get("/", (req, res) => {
  res.send("hello from nodeJS!");
});

app.listen(7211, () => {
  console.log("listening on 7211");
});
```

```json
// package.json
{
  "name": "scapenode",
  "main": "index.js",
  "scripts": {
    "start": "node ./src/index.js"
  },
  "dependencies": {
    "express": "^4.17.1"
  }
}
```

To onboard this application to the platform, you must create a `shipwreck.toml` file in the root directory of your repository, in the branch you want to be deployed to the platform. An example might look like the following:

```toml
[app]
name="scapenode"
version="1.0.0"
author="Ethan Shry"
endpoint="https://github.com/ethanshry/scapenode"

[config]
port=7211 # the port specified in the express server
lang="node" # node specified that this is a nodejs application
run="npm start" # the command to be run - we could replace this with `node ./src/index.js`
```

Which will yield the following directory structure:

```bash
.
├── package.json
├── shipwreck.toml
└── src
    └── index.js
```

Components of the `app` section describe metadata about the application, while fields of the `config` section describe important configuration aspects of your deployment. More details about these fields are available on the `info` screen in the platform interface, and the config field format is implemented in the `crate::deployment::shipwreck` module.

You will then need to access the platform interface to deploy the application, which means you need the IP address of the machine which is running the platform. If do not know the IP address, the [Kraken-Orchestrator-Discovery](https://github.com/ethanshry/Kraken-Orchestrator-Discovery) tool will scan your LAN for an IP address with an exposed port `8000`, which is the port that the platform API runs on. While a DNS server for the platform was something which was explored for ease-of-use, traditional DNS implementations require a consistent IP address and configuring either all devices on your LAN or your Router to point to that IP address, and mDNS libraries for Rust were incompatible, incomplete, or generally not sufficiently mature to be implemented in a reasonable amount of time. In any case, navigate to the platform interface at `PLATFORM_IP:8000`.

From there, you simply need to access the `deployments` tab of the interface, and select the appropriate URL (NOT the git clone URL) and branch for your deployment. This should enable the `Create Deployment` button, which will delegate the deployment to a worker on the platform.

![Deployment UI Example](./images/deployment_url_and_branch_ok.png)

## IV. Platform Systems

This section will cover the main systems involved in the platform. The figure below outlines the major responsibilities of a node on a platform.

![Node Components](./images/node_components_diagram.png)

As you can see, each node has two different `Executors` running at any given time- a `WorkerExecutor` and an `OrchestrationExecutor`. These are outlined more in the [Executors](###Executors) section, however an executor is a way to differentiate groups of functionality. There are effectively two types of nodes on the platform- orchestrators and workers. Orchestrators are either active (the primary orchestrator) or inactive (rollover candidates). Their responsibilities are covered in the [Workers](###Workers), [Orchestrators](###Orchestrators), and [Orchestration Rollover Candidates](###Orchestration-Rollover-Candidates) sections below.

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
        match orchestrator.execute().await {
            Ok() => {}
            Err(failure) => match failure {
                // handle orchestrator failures
                // this might involve orchestration rollover
                // ...
            },
        };
        match worker.execute().await {
            Ok() => {}
            Err() => {
                // handle various forms of ExecutionFailure
                // ...
            }
        };
    }
}

```

### Executors

There are a variety of tasks a `Node` on the platform must perform. These broadly fall into two categories: features required of a `Worker`, and features required of an `Orchestrator`. While the specific responsibilities for each role will be outlined in the [Workers](###Workers), [Orchestrators](###Orchestrators), and [Orchestration Rollover Candidates](###Orchestration-Rollover-Candidates) sections, all of these roles follow the same `Executor` trait.

An executor is simply a struct which implements two methods: setup and execute. `setup` is called once to set up the node, and `execute` will then be called repeatedly until the executor crashes or the program terminates. Both of these methods also accept a `GenericNode`, which allows different executors to share common data, and return the same `SetupFailure` and `ExecutionFailure` responses across executors. This allows us to isolate different functionality to specific executors, while having a single API to interface with them. As a result of this design, it would be trivial to add another executor to extend the functionality of our `Node` without impacting existing executors, or running a `Node` which is only has an `Orchestrator` or a `Worker` without being both.

```rust
pub trait Executor {
    /// Is called once to set up this node
    async fn setup(&mut self, node: &mut GenericNode) -> Result<(), SetupFailure>;
    /// Is called repeatedly after setup has terminated
    async fn execute(&mut self, node: &mut GenericNode) -> Result<(), ExecutionFailure>;
}

pub struct GenericNode {
    pub broker: Option<RabbitBroker>,
    pub system_id: String,
    pub rabbit_addr: String,
    pub orchestrator_addr: String,
}
```

### Workers

A Worker, or more precisely a `WorkerExecutor`, handles all tasks in relation to actually deploying and monitoring applications. This includes almost all of the interface between the platform and the Docker Engine.

```rust
pub struct WorkerExecutor {
    work_queue_consumer: Option<lapin::Consumer>,
    deployments: std::collections::LinkedList<DeploymentInfo>,
    tasks: Vec<Task>,
}
```

A `WorkerExecutor` is relatively simple- it has a handle to a RabbitMQ Queue, `work_queue_consumer`, over which it receives `WorkRequestMessages`. It has a list of deployments it monitors, `deployments`, and it has a handle to other tasks it is monitoring, `tasks`.

When setup is called on the `WorkerExecutor`, it connects to RabbitMQ, begins broadcasting the Node status to the orchestrator, and establishes the consumer for the `work_queue_consumer`.

On a call to `execute`, the worker does two things: first, it checks to see if it has an outstanding `WorkRequestMessage`. This message is passed to the worker via the RabbitMQ consumer and is extensible, but currently only supports two request types: `RequestDeployment` and `CancelDeployment`. The request contains all the necessary information for the Worker to perform the requested task. Finally, the `Worker` looks at all its active deployments and broadcasts any relevant log messages to the relevant RabbitMQ queue. More information about how the `RequestDeployment` process works can be found in [Deployments](###Deployments)

Below you can find a simplified version of the code which implements the `Executor` trait for the `WorkerExecutor`.

```rust

impl Executor for WorkerExecutor {

    async fn setup(..) -> Result<(), SetupFailure> {
        // connect to rabbitMQ
        connect_to_rabbit_instance(&node.rabbit_addr).await;

        // create a task to monitor this Node's stats
        WorkerExecutor::get_publish_node_system_stats_task(node).await

        // establish consumer for the worker's work queue
        broker.consume_queue_incr(&node.system_id).await
    }

    async fn execute(..) -> Result<(), ExecutionFailure> {
        // if we have a WorkRequestMessage, then execute it
        if let Some(data) = try_fetch_consumer_item(&mut self.work_queue_consumer).await {
            let (_, task) = WorkRequestMessage::deconstruct_message(&data);

            match task.request_type {
                RequestDeployment => {
                    handle_deployment(&task).await;
                }
                CancelDeployment => {
                    kill_deployment(&task).await;
                }
            }
        }

        for (index, d) in self.deployments {

            let logs = docker.get_logs(&d.deployment_id).await;
            if !logs.is_empty() {
                let mut msg = LogMessage::new(&d.deployment_id, &logs.join());
                msg.send().await;
            }

            let status = docker.get_container_status(&d.deployment_id).await
            let mut msg = DeploymentMessage::new(&d.deployment_id, status);
            msg.send().await;
        }
    }
}

```

### Orchestrators

An Orchestrator, or more precisely an `OrchestratorExecutor`, is more complex. It handles all tasks in relation to coordinating deployments- this includes receiving requests for deployments/cancellations from the REST API, validating they are deployable, and distributing requests to nodes. Additionally the orchestrator is responsible for deploying and responding to requests from the REST and GraphQL APIs, deploying the RabbitMQ instance, storing application logs, and otherwise managing the platform.

```rust
pub struct OrchestrationExecutor {
    /// A handle to the rocket.rs http server task
    api_server: Option<tokio::task::JoinHandle<()>>,
    /// A reference to the Database containing information about the platform
    pub db_ref: Arc<Mutex<Database>>,
    /// The rank of this executor for rollover.
    /// 0 implies this is the active orchestrator, None implies no priority is assigned.
    /// Otherwise is treated as lowest number is highest priority
    pub rollover_priority: Option<u8>,
    pub queue_consumers: Vec<Task>,
}
```

The most important piece of the `OrchestrationExecutor` is the `db_ref`. This is a thread-safe reference to a `Database`. See more information in [In-Memory State Storage](###In-Memory-State-Storage) and [Communication](###Communication) for how this works, but the general idea is the `api_server` has a reference to the database, and so is able to create requests there for deployments. The Orchestrator is then able to read the `Database` record for Nodes and Deployments, and take any actions necessary based on their states.

When setup is called on the `OrchestrationExecutor`, it fetches the most recent version of the `Kraken-UI` interface and compiles it, spins up a `RabbitMQ` server, connects to it, and sets up threads to consume the various status queues. Finally it establishes the REST and GraphQL APIs which serve the UI and Platform Data.

On a call to `execute`, the Orchestrator does two things: first, it checks all available deployment statuses to see if there is any action to be taken. For example, a deployment might be asking to be updated, destroyed, or created. The Orchestrator will take any action necessary for those deployments. Then, the Orchestrator will check all the `Nodes` it knows about to ensure they have recently reported a status. If they haven't, it will re-deploy any deployments owned by that Node, and otherwise remove it from the platform.

Below you can find a simplified version of the code which implements the `Executor` trait for the `OrchestrationExecutor`.

```rust

impl Executor for OrchestrationExecutor {
    async fn setup(..) -> Result<(), SetupFailure> {

        // Setup RabbitMQ server and Fetch and compile Kraken-UI
        let ui = OrchestrationExecutor::fetch_ui();
        let rabbit = OrchestrationExecutor::deploy_rabbit_instance();

        Self::connect_to_rabbit_instance().await;

        // Consume RabbitMQ Queues
        // Queue for information about Node statuses
        self.get_sysinfo_consumer();

        // Queue for information about Deployment statuses
        self.get_deployment_consumer();

        // Queue for receiving Deployment Logs
        self.get_log_consumer();

        OrchestrationExecutor::create_api_server();
    }

    async fn execute(..) -> Result<(), ExecutionFailure> {

        let deployments = db.get_deployments();

        for mut deployment in d {
            match deployment.status.0 {
                DeploymentRequested => {
                    // Look for free nodes to distribute tasks to
                    deployment.update_status(ValidatingDeploymentData);
                    if let Err(_) = validate_deployment(..) {
                        continue
                    }
                    deployment.update_status(DelegatingDeployment);

                    // Pick the best node to handle the deployment
                    // ..

                    deployment.update_status(Deploying);
                    // Send work to curr_node
                    // Create and send a WorkRequestType::RequestDeployment
                    // For the appropriate node
                    // ..
                }
                UpdateRequested => {
                    let commit = github_api::get_tail_commit(..);
                    if commit != deployment.commit {
                        // Creates and sends a WorkRequestMessage::CancelDeployment
                        // followed by a WorkRequestType::RequestDeployment
                        // ..
                    }
                }
                DestructionRequested => {
                    // Creates and sends a WorkRequestMessage::CancelDeployment
                    // ..
                }
            }
        }

        for n in db.get_nodes() {
            // Check to see if a node hasn't been heard from in awhile
            // If so, then re-deploy all it's owned deployments and remove
            // All node information from the DB
            // ..
        }
    }
}

```

### Orchestration Rollover Candidates

Though technically this role falls under the `OrchestrationExecutor` umbrella, an Orchestration Rollover Candidate is worth talking about separately. These are `OrchestrationExecutors` which have a `rollover_priority != Some(0)`. In these cases, the `setup` for the executor will do nothing. In `execute`, the orchestrator is only responsible for checking to see that it can communicate with the primary orchestrator. If the candidate is the primary candidate (i.e. the first rollover node), then it will also backup the database and log data from the primary orchestrator. For more information on the rollover process itself, see [Rollover](##Rollover)

```rust

impl Executor for OrchestrationExecutor {
    async fn setup(..) -> Result<(), SetupFailure> {
        // We are not the primary orchestrator, so no setup is necessary
    }

    async fn execute(..) -> Result<(), ExecutionFailure> {

        let priority = get_rollover_priority(..).await;
        match priority {
            None => {
                // No priority means we have an issue with the orchestration communication
                // Here we attempt some recovery
                // ..
            }
            Some(_) => {
                if self.rollover_priority == Some(1) {
                    // We are the primary rollover candidate, so we need to be backing up
                    // data from the primary
                    let database_data = get_primary_orchestrator_db_data(..);
                    db.clear();
                    db.backup(database_data)

                    for log in logs_to_backup {
                        backup_log_file(&log).await;
                    }
                }
                return Ok(());
            }
        }
    }
}

```

### Communication

The communication between different platform systems is designed to maximize simplicity within each system. The below diagram covers the different pieces of the platform, and how they are communicating.

![Platform Communication](./images/platform_communication_diagram.png)

The first thing to note is the way external users interface with the platform. The User Interface only communicates with the platform via the GraphQL API. This communication method was chosen so as to maximize flexibility in development- by using GraphQL, it is very easy to modify the schema of the requests on-the-fly, which sped up development significantly, and allows for future expansibility. The GraphQL API (which actually rests on top of a standard REST API) has a thread-safe reference to the In-Memory State Storage (or the `Database`) that is owned by the `OrchestrationExecutor`. All the data to power the API comes from this database, and any user requests which are made to the platform will be created in this database. The only exception to this is the delivery of UI files (and application log files), which come from the REST API.

This theme- the ownership of a thread-safe reference to the `OrchestrationExecutor`'s database, is common across all other parts of the platform as well. The `OrchestrationExecutor` only monitors the database to determine which tasks to perform. The different RabbitMQ queue consumers the database owns all have a reference to this database as well, so any inbound to the `OrchestrationExecutor` comes through the database. The only exception to this is in Orchestration Rollover Candidates, which monitor a route on the REST API to ensure the primary orchestrator is still active, and to receive their backup data.

When an `OrchestrationExecutor` has a need to communicate with a `WorkerExecutor` (in either direction), that communication will travel through the RabbitMQ instance. This includes the following messages:

| Channel ID   | Direction | Message Struct     | Description                                                                                   |
| ------------ | --------- | ------------------ | --------------------------------------------------------------------------------------------- |
| Sysinfo      | W -> O    | SysinfoMessage     | Carries information about the status of devices on the network                                |
| Deployment   | W -> O    | DeploymentMessage  | Carries information about a deployment (either in build, while active, or after deletion)     |
| Log          | W -> O    | LogMessage         | Carries the log information from a deployment (currently only while the deployment is active) |
| \<SystemId\> | O -> W    | WorkRequestMessage | Allows the orchestrator to request work from a worker                                         |

### In-Memory State Storage

There has been a fair bit of discussion about this already, however the primary `OrchestrationExecutor`'s `Database` is the single source of truth for the platform.

```rust
pub struct Database {
    /// Information about the deployments the orchestrator is managing
    deployments: HashMap<String, Deployment>,
    /// Information about the nodes the orchestrator is managing
    nodes: HashMap<String, Node>,
    /// Information about the platform
    orchestrator: Orchestrator,
}
```

Anything which wants to communicate inbound to the `OrchestrationExecutor` (i.e. RabbitMQ Consumers and the REST/GraphQL APIs) has an `Arc<Mutex<Database>>` to this database. This model is actually very similar to the implementation of the [mini-redis](https://github.com/tokio-rs/mini-redis)[6] crate, which is both simple and designed by the creators of `tokio`, one of the most popular async runtimes for Rust. This design allows the `OrchestrationExecutor::execute` method to iterate over information in the database, instead of looking for messages across many different RabbitMQ queues, or performing many operations out-of-band as the different messages come in from different sources. While this does potentially reduce performance, since we look at each deployment through each iteration, this reduces the potential for the Orchestrator to act on out-of-date information, and the simplicity and ease of debugging gained by having a single straightforward execution loop is very convenient.

### Rollover

The rollover process for the platform is actually relatively straightforward. All rollover activities are handled by the `OrchestrationExecutor` and dictated by the `rollover_priority`. Orchestrators have four states:

| `rollover_priority` | Meaning                                                                                                                                                   |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Some(0)`           | The node is the primary orchestrator                                                                                                                      |
| `Some(1)`           | The node is the primary rollover candidate, and is backing up the database and log files of the primary orchestrator                                      |
| `Some(255)`         | The node is not yet in the database, so it cannot be assigned a priority, or all priorities are assigned                                                  |
| `Some(n)`           | The node is not a rollover candidate, and just monitors the health endpoint. It could become the primary candidate as a result of a healthcheck api call. |

Priorities are assigned by calls to the `/health/<requester_node_id>` API endpoint, which is the endpoint used by secondary orchestrators to ensure the primary orchestrator is running, through the following method:

```rust
pub fn get_orchestrator_rank(&mut self, node_id: &str) -> u8 {
    if let Some(n) = self.nodes.get(node_id) {
        let mut n = n.clone();
        // node does not have an assigned priority, so figure one out
        let nodes: Vec<_> = self.nodes.iter().collect();
        let mut priorities: [bool; 255] = [false; 255];
        for (_, n) in nodes {
            if let Some(p) = n.orchestration_priority {
                priorities[p as usize] = true;
            }
        }
        for (i, v) in priorities.iter().enumerate() {
            if *v == false {
                if let Some(p) = n.orchestration_priority {
                    if p < (i as u8) {
                        return p;
                    }
                }
                n.orchestration_priority = Some(i as u8);
                self.nodes.insert(node_id.to_string(), n);
                return i as u8;
            }
        }
        // No priority is available
        return 255;
    } else {
        // node is not in DB, so give no priority
        return 255;
    }
}
```

In the case that a node which is not the Orchestrator (that is to say, with a `rollover_priority != Some(0)`) is unable to access the `/health/<requester_node_id>` REST API endpoint, that executor will go into an exponential backoff period attempting to regain access to that route. That is to give the primary orchestrator time to recover from whatever service interruption it is experiencing. If after that period of backoff (~10s) the secondary orchestrator is unable to establish a connection, it will return an `ExecutionFailure::NoOrchestrator` from it's `execute` method call. When this response is received by the main application loop, the orchestrator will then do one of two things. If it is the primary rollover candidate (i.e. `rollover_priority == Some(1)`) then we will alter our rollover priority to be `Some(0)` and establish itself as the primary orchestrator of a new platform. All other nodes will instead enter another period of exponential backoff (~3m) waiting to connect to the new platform, at the end of which they will terminate.

In the case of the failure of a node to report its status to the Orchestrator for a period of 1 minute, the Orchestrator will remove it from the database, and redeploy its deployments to other nodes. Additionally, that node's orchestrator's `rollover_priority` will be freed up to be assigned to another node.

This rollover scheme is sufficient to handle the loss of any number of worker nodes, since whenever a worker node is lost, the remaining node's `rollover_priorities` will be reassigned. In the case that a worker node with `rollover_priority == Some(1)` is lost and within the minute of no connection, the Orchestrator goes down, the platform will have cascading node failure. In the case that the Orchestrator goes down, and the primary rollover candidate also goes down before it has a chance to assign a new primary rollover candidate, the platform will also have cascading node failure. While this would be a terrible design for a production system, the primary focus of the platform is its ease-of-use, not its rollover strategy, and it is relatively easy to just reboot the platform and re-establish the deployments. Additionally the case in which multiple specific devices would fail within minutes of each other, due to non-platform induced problems (since they would need to fail almost immediately after setup to not have assigned backup nodes) is relatively low.

In the case of rollover, the new orchestrator will be operating on the most recently saved `Database` backup. Since the platform was designed as a development environment, we do not make any effort to re-attach to existing deployments. Instead, we send a signal to all nodes to spin down their deployments, and the primary orchestrator will treat all deployments as though they were newly requested.

### User Interface

The platform UI is relatively simple. It provides basic information about the platform, including a brief tutorial for onboarding. It has a page to monitor the various nodes attached to the platform, and a place to request and monitor the various deployments on the platform.

The User Interface is a React-Typescript app. It used Urql as the GraphQL API interface, and Ant-Design as the UI library.

It is not based on Create-React-App, which is probably the standard way to setup a new React project, and handles building and minimizing a react project, as the default set of dependencies from Create-React-App as of March 2021 is over 50 items, which felt egregious for a project boilerplate. Instead, Kraken-UI uses [Parcel](https://parceljs.org/getting_started.html)[7] as a bundler, which is a low-configuration bundler.

While it is nice to not have the overhead of Create-React-App, or the complexity of Webpack, Parcel is currently transitioning from v1 to v2, and Ant-Design and Parcel do not play well together. If I were to do this project over, I would not use both together (probably transitioning to a different UI library, and back to Webpack as a bundler unless Parcel has had sufficient time to mature).

## Future Work

While the project at this point does have a complete set of features, there are a few things which would make it have significantly more utility:

- Some form of DNS support (possibly mDNS) so applications can be accessed consistently without needing to find their IPs
- Database deployments (especially that could be linked to deployments for testing purposes)
- Built-in support for ENV variable injection into deployments (i.e. API Keys). This is currently done for the Kraken-UI compilation, so likely would be relatively low effort

DNS resolution was something which should have been part of this project, however at that stage of development there were not libraries which were stable enough/compatible with other libraries in this project and the nightly build of the compiler, so due to the amount of time going into this feature it had to be scrapped.

## Artifacts

| Artifact Name                 | Artifact Use                         | Link                                                                                                                     |
| ----------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| Kraken                        | Platform/Backend Code                | [https://github.com/ethanshry/Kraken](https://github.com/ethanshry/Kraken)                                               |
| Kraken-UI                     | UI/Frontend Code                     | [https://github.com/ethanshry/Kraken-UI](https://github.com/ethanshry/Kraken-UI)                                         |
| Kraken-Orchestrator-Discovery | Orchestrator IP Discovery Tool       | [https://github.com/ethanshry/Kraken-Orchestrator-Discovery](https://github.com/ethanshry/Kraken-Orchestrator-Discovery) |
| Kraken-Utils                  | Rust Crate to support Kraken Backend | [https://github.com/ethanshry/Kraken-Utils](https://github.com/ethanshry/Kraken-Utils)                                   |

## Acknowledgements

I want to personally thank a few different sources for making this project possible:

First, I want to thank the creators of [The Rust Book](https://doc.rust-lang.org/book/)[8]. This is truly worth reading for every new rust developer, and was immensely helpful in understanding the basics.

The r/rust reddit community was also helpful, and significantly better than StackOverflow to provide understanding of both language features and the history behind how those features came to be.

Jon Gjenset[9] and Ryan Levick[10] are both Rust developers who pulish long-form YouTube content about Rust, which was useful and interesting to better understand some of the inner workings of the language.

Finally, I want to thank Bill Siever for allowing me to run with a random project idea and for being flexible enough to let me shape it as I go, I've had a great time being able to explore the areas I found interesting and appreciate the opportunity to make something I actually want to build the way I want to build it.

## References

- \[1\] - Cloud Adoption Survey https://resources.idg.com/download/2020-cloud-computing-executive-summary-rl
- \[2\] - AWS Pricing https://aws.amazon.com/ec2/pricing/
- \[3\] - Azure Pricing https://azure.microsoft.com/en-us/pricing/details/virtual-machines/linux/
- \[4\] - GCP PRicing https://cloud.google.com/compute/vm-instance-pricing
- \[5\] - Kubernetes Challenges in Industry https://kublr.com/industry-info/docker-and-kubernetes-survey/
- \[6\] - Mini-Redis https://github.com/tokio-rs/mini-redis
- \[7\] - Parcel JS https://parceljs.org/getting_started.html
- \[8\] - The Rust Book https://doc.rust-lang.org/book/
- \[9\] - Jon Gjenset's Youtube Channel https://www.youtube.com/channel/UC_iD0xppBwwsrM9DegC5cQQ
- \[10\] - Ryan Levick's Youtube Channel https://www.youtube.com/channel/UCpeX4D-ArTrsqvhLapAHprQ
