# Random Snippets which might be useful examples

```rust

// Will error since &d is partially captured by db.insert_deployments
let mut log_request_ids = vec![];
if let Some(orch_db) = database_data {
    if let Some(deployments) = orch_db.get_deployments() {
        for mut d in deployments {
            log_request_ids.push(d.id);
            db.insert_deployment(&d);
        }
    }
}

let mut log_request_ids = vec![];
if let Some(orch_db) = database_data {
    if let Some(deployments) = orch_db.get_deployments() {
        for mut d in deployments {
            log_request_ids.push(d.id.clone());
            db.insert_deployment(&d);
        }
    }
}

```

```rust

// this thing

 None => {
    if *time == *(backoff.last().clone().unwrap_or(&55)) {
        // We have failed to find an orchestrator
        error!("Failed to find a new orchestrator");
        panic!("No orchestrator, exiting");
    }
}

```

````rust

/// This code from db feels super clean
/// Deletes the specified node and marks all its owned deployments for re-deploy
    pub fn delete_node(&mut self, node_id: &str) -> Option<Node> {
        if let Some(_) = self.get_node(node_id) {
            if let Some(ds) = self.get_deployments() {
                for mut d in ds {
                    if d.node == node_id {
                        d.node = String::from("");
                        d.update_status(&ApplicationStatus::DeploymentRequested);
                        self.update_deployment(&d.id, &d);
                    }
                }
            }
            return self.nodes.remove(node_id);
        }
        None
    }
    ```
````

```rust
// Annoying string matching stuff
let ids = docker
                .start_container(
                    &r.image_id,
                    port,
                    match &dockerfile_name.unwrap_or("")[..] {
                        "static" => Some(80),
                        _ => None,
                    },
                )
                .await;

```

```rust
// get_container_status from docker mod.rs
let mut status = ContainerStatus {
                    state: i
                        .state
                        .unwrap()
                        .status
                        .unwrap_or(bollard::models::ContainerStateStatusEnum::DEAD),
                    size: i.size_root_fs.unwrap_or(0) as i32,
                    mem_mb: 0,
                    mem_max_mb: 0,
                    cpu_usage: 0.0,
                };

```
