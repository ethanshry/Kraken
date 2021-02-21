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
