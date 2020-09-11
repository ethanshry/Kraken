# Deployment

The core functionality of this platform is the Application Deployment functionality- sending a Github URL to be deployed to the platform, and monitoring that URL for changes (automatically redeploying when a change is detected).

## Configuration

The deployment process looks for a `Config.toml` file in the root of the cloned repository. Repositories which do not contain this file will immidiately exit the deployment process. The Config.toml should contain key information in reguards to the deployment- in particular:

- The Domain name your application will be deployed under
- The Port your app will attempt to use
- Which dockerfile the platform should use

Other information, and the exact specification for the file fields and format, are still in progress.

An example of what the current file format is is shown below:

```toml
[app]
name="scapegoat"
version="1.0.0"
author="Ethan Shry"
endpoint="https://github.com/ethanshry/scapegoat"

[config]
lang="python3"
test=""
run="python3 ./src/main.py"


[env-vars]
test-var="test-var content"
```

## Deployment Process

Deployment is kicked off by a GraphQL call to the database. This will cause the database to insert a new `Deployment` entity with the `crate::model::ApplicationStatus::REQUESTED`.

The Orchestrator's execution process involves checking the status of all active deployments. In the case that an `ApplicationStatus::REQUESTED` is detected, the deployment status will be updated to `ApplicationStatus::INITIALIZED`. The Orchestrator will then seek for a node which will be responsible for the deployment, and will send a message on that node's RabbitMQ Work Queue.

When a Node recieves a message on its Work Queue, it will take action on that Work Request. In the case of a deployment, the Node will kick off the Deployment Action, which will use the deployment information to request the application from Git and deploy the application locally via Docker, all the while sending status information back to the Orchestrator via the `Deployment` RabbitMQ Queue.
