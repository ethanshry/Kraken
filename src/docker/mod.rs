//! Interface between rust and docker engine (either via bollard or the CLI)

use bollard::image::ListImagesOptions;
use bollard::{
    container::{
        Config, CreateContainerOptions, ListContainersOptions, LogOutput, LogsOptions,
        PruneContainersOptions, StartContainerOptions, StopContainerOptions,
    },
    image::{BuildImageOptions, PruneImagesOptions},
    service::{HostConfig, PortBinding},
    Docker,
};
use flate2::write::GzEncoder;
use flate2::Compression;
use futures_util::stream::StreamExt;
use futures_util::stream::TryStreamExt;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::time::SystemTime;
use uuid::Uuid;

pub mod docker_container;

use docker_container::DockerContainer;

/// The interface between Kraken and Docker
/// The Docker Broker connects to the Docker Engine socket via the bollard crate
/// Kraken can then use this connection to interface with the Docker engine.
pub struct DockerBroker {
    /// Connection to the Docker Engine socket
    pub conn: bollard::Docker,
}

impl DockerBroker {
    /// Attempts to connect to the Docker Engine, and if a connection can be established, returns a DockerBroker
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// if let Some(d) = docker {
    ///     println("Connected to docker!")
    /// }
    /// ```
    pub async fn new() -> Option<DockerBroker> {
        let conn = Docker::connect_with_unix_defaults();
        match conn {
            Ok(c) => {
                let version = c.version().await.unwrap();
                match version.version {
                    Some(v) => info!("Docker {} connection established", v),
                    None => info!("Docker [unspecified version] connection established"),
                }
                Some(DockerBroker { conn: c })
            }
            Err(e) => {
                error!("Error establishing conn: {:?}", e);
                None
            }
        }
    }

    /// Gets a list of existing docker images
    ///
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// if let Some(d) = docker {
    ///     let ids = d.get_image_ids();
    ///     for id in ids {
    ///         println!("{:?}", id);
    ///     }
    /// }
    /// ```
    pub async fn get_image_ids(&self) -> Vec<String> {
        let images = self
            .conn
            .list_images(Some(ListImagesOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await
            .unwrap();

        let mut ids = vec![];

        for image in images {
            info!("-> {:?}", image);
            ids.push(image.id);
        }
        ids
    }

    /// Fetches logs for a specific Docker image starting from the specified time
    /// Returns a Vec<String> of log entries
    ///
    /// # Arguments
    ///
    /// * `container_id` - The id of the docker container to access
    /// * `time` - The system time to start fetching logs from
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// if let Some(d) = docker {
    ///     let ids = d.get_image_ids();
    ///     for id in ids {
    ///         // to fetch logs starting from the current time (should be empty)
    ///         let log = d.get_logs(id, SystemTime::now());
    ///         for line in log {
    ///             println!("{}", line);
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn get_logs(&self, container_id: &str, time: SystemTime) -> Vec<String> {
        let options = Some(LogsOptions::<String> {
            stdout: true,
            stderr: true,
            timestamps: true,
            since: i64::try_from(
                time.duration_since(SystemTime::UNIX_EPOCH)
                    .expect("Bad Duration for System Logs")
                    .as_secs(),
            )
            .unwrap_or(0),
            ..Default::default()
        });
        let mut stream = self.conn.logs(container_id, options);
        let mut log_items = vec![];

        while let Some(item) = stream.next().await {
            match item {
                Ok(m) => {
                    info!("{}", m);
                    match m {
                        LogOutput::StdOut { message: m } | LogOutput::Console { message: m } => {
                            log_items
                                .push(format!("[LOG]: {}", std::str::from_utf8(&m).unwrap_or("")));
                        }
                        LogOutput::StdIn { message: m } => {
                            log_items
                                .push(format!("[IN]: {}", std::str::from_utf8(&m).unwrap_or("")));
                        }
                        LogOutput::StdErr { message: m } => {
                            log_items
                                .push(format!("[ERR]: {}", std::str::from_utf8(&m).unwrap_or("")));
                        }
                    }
                }
                Err(_) => {}
            }
        }

        log_items
    }

    /// Gets a list of running docker containers
    ///
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// let containers = docker.get_running_containers();
    /// for c in container {
    ///     println!("{:?}", c);
    /// }
    /// ```
    pub async fn get_running_containers(&self) -> Vec<DockerContainer> {
        let cs = self
            .conn
            .list_containers(Some(ListContainersOptions::<String> {
                ..Default::default()
            }))
            .await
            .unwrap();

        let mut containers = vec![];

        for c in cs {
            let id = c.id.unwrap();
            let name = match c.names {
                // Get first name from vector and drop first character (they all start with /?)
                Some(names) => String::from(&names[0][1..]),
                None => String::from(""),
            };
            let ports = match c.ports {
                Some(ps) => {
                    let mut v = vec![];
                    for p in ps {
                        if let Some(port) = p.public_port {
                            v.push(port);
                        }
                    }
                    Some(v)
                }
                None => None,
            };
            containers.push(DockerContainer::new(
                id, name, c.image, c.image_id, c.created, ports, c.state, c.status,
            ));
        }

        containers
    }

    /// Builds a docker image from a local project folder
    ///
    /// This will create a `/tmp/containers` directory if it doesn't exist to store a tar of the project before building the image.
    /// # Arguments
    ///
    /// * `source_path` - The path relative to the root of the crate which contains the desired image contents. A `Dockerfile` is expected to be in this folder.
    /// * `id` - An id for the container, if one is not provided, it will be generated.
    ///
    /// # Returns
    ///
    /// * `Result<DockerImageBuildResult, String>` -  Returns a DockerImageBuildResult on succesful build, or a string description of the error otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// docker.build_image("./tmp/test-proj", Some(12345)); // builds image 12345 from the specified directory
    /// ```
    pub async fn build_image(
        &self,
        source_path: &str,
        id: Option<String>,
    ) -> Result<DockerImageBuildResult, String> {
        let container_guid = id.unwrap_or(Uuid::new_v4().to_hyphenated().to_string());
        // tar the directory
        let make_tar = || -> Result<(), std::io::Error> {
            // Create directory tree if it doesn't exist
            fs::create_dir_all("./tmp/containers")?;
            let tar_gz = File::create(format!("./tmp/containers/{}.tar.gz", &container_guid))?;
            let enc = GzEncoder::new(tar_gz, Compression::default());
            let mut tar = tar::Builder::new(enc);
            tar.append_dir_all(".", source_path)?;
            tar.into_inner()?;
            Ok(())
        };
        match make_tar() {
            Ok(_) => {
                let mut log: Vec<String> = vec![];
                info!("Tar for {} completed succesfully", source_path);
                log.push(format!(
                    "[KRAKEN][INFO]Tar for {} completed succesfully\n",
                    source_path
                ));
                let build_result: Result<(), String> = async {
                    let mut file =
                        File::open(format!("./tmp/containers/{}.tar.gz", &container_guid))
                            .expect("Could not find tarball");
                    let mut contents = Vec::new();
                    file.read_to_end(&mut contents)
                        .expect("Failed to read tarball");

                    info!("Building docker image [{}]", &container_guid);
                    log.push(format!(
                        "[KRAKEN][INFO]Building docker image [{}]\n",
                        &container_guid
                    ));
                    let mut build_results = self.conn.build_image(
                        BuildImageOptions {
                            dockerfile: "Dockerfile",
                            t: &container_guid,
                            rm: true,
                            networkmode: "bridge",
                            ..Default::default()
                        },
                        None,
                        Some(contents.into()),
                    );

                    while let Some(result) = build_results.next().await {
                        if let Ok(stage) = result {
                            let bollard::service::BuildInfo {
                                id,
                                stream: _,
                                error,
                                error_detail: _,
                                status,
                                progress,
                                progress_detail,
                                aux: _,
                            } = stage;
                            info!(
                                "{:?},{:?},{:?},{:?},{:?}",
                                id, error, status, progress, progress_detail
                            );
                            let data = "";
                            if !data.is_empty() {
                                log.push(format!("[KRAKEN][INFO] {}", data.to_owned()));
                            }
                        /*
                        _ => {
                            // TODO figure out what to do with other results
                            // BuildImageAux is called right before the final Stream message
                        }
                        */
                        } else {
                            // TODO figure out what to do with Err
                        }
                    }
                    Ok(())
                }
                .await;
                match build_result {
                    Ok(_) => Ok(DockerImageBuildResult {
                        log,
                        image_id: container_guid.clone(),
                    }),
                    Err(e) => {
                        error!("Error building container {}: {}", &container_guid, &e);
                        Err(String::from("Failed to build image"))
                    }
                }
            }
            Err(e) => {
                error!("Failed to tar source from path {}", source_path);
                Err(format!("{:?}", e))
            }
        }
    }

    /// Both creates and starts a docker container
    ///
    /// # Arguments
    ///
    /// * `image_id` - The id of the image to turn into a container
    /// * `host_port` - A port that will be exposed publicly on the node.
    /// * `container_port` - The port which should be exposed on the docker container. If None, will map host_port -> container port
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// docker.start_container("12345", 9000, None); // starts image 12345 and maps docker:9000->host:9000
    /// docker.start_container("12345", 9000, Some(500)); // starts image 12345 and maps docker:500->host:9000
    /// ```
    pub async fn start_container(
        &self,
        image_id: &str,
        host_port: i64,
        container_port: Option<i64>,
    ) -> Result<String, ()> {
        // TODO support exposing multiple ports? Check out TCP vs UDP?
        let mut ports = HashMap::new();

        let p = format!("{}/tcp", host_port);

        ports.insert(&p[0..p.len()], HashMap::new());

        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            match container_port {
                Some(p) => format!("{}/tcp", p),
                None => p.clone(),
            },
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}", host_port)),
            }]),
        );

        let config = Config {
            hostname: Some("example-service.dev"), // TODO doesn't work right now
            image: Some(image_id),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            exposed_ports: Some(ports),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                network_mode: Some(String::from("bridge")), // TODO doesn't work right now
                ..Default::default()
            }),
            ..Default::default()
        };

        info!("{:?}", config);

        let res = self
            .conn
            .create_container(Some(CreateContainerOptions { name: image_id }), config)
            .await;

        info!("{:?}", res);

        if let Ok(response) = res {
            info!("Docker built container {}", response.id);
            let res = self
                .conn
                .start_container(&response.id, None::<StartContainerOptions<String>>)
                .await;
            if res.is_ok() {
                info!("Docker started container {}", response.id);
                return Ok(response.id);
            }
            return Err(());
        }
        Err(())
    }

    /// Stops a docker container
    ///
    /// # Arguments
    ///
    /// * `container_id` - The id of the container to kill
    pub async fn stop_container(&self, container_id: &str) {
        self.conn
            .stop_container(container_id, Some(StopContainerOptions { t: 10 }))
            .await
            .unwrap_or_else(|_| {});
        info!("Killing docker container {}", container_id);
    }

    pub async fn get_container_status(&self, container_id: &str) -> Option<ContainerStatus> {
        // container_name and container id are interchangable for inspect_container
        let inspect = self.conn.inspect_container(container_id, None).await;
        match inspect {
            Ok(i) => {
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
                self.conn
                    .stats(
                        container_id,
                        Some(bollard::container::StatsOptions { stream: false }),
                    )
                    .take(1)
                    .map(|value| match value {
                        Ok(stats) => {
                            status.mem_mb =
                                (stats.memory_stats.usage.unwrap_or(0) / 1_000_000) as i32;
                            status.mem_max_mb =
                                (stats.memory_stats.max_usage.unwrap_or(0) / 1_000_000) as i32;
                            // see https://github.com/moby/moby/blob/eb131c5383db8cac633919f82abad86c99bffbe5/cli/command/container/stats_helpers.go#L175
                            // for calculation details
                            status.cpu_usage = ((stats.cpu_stats.cpu_usage.total_usage as f32)
                                - (stats.precpu_stats.cpu_usage.total_usage as f32))
                                / ((stats.cpu_stats.system_cpu_usage.unwrap() as f32)
                                    - (stats.precpu_stats.system_cpu_usage.unwrap() as f32))
                                * (stats.cpu_stats.cpu_usage.percpu_usage.unwrap().len() as f32)
                                * 100.0 as f32;
                            Ok(())
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    })
                    .try_collect::<Vec<()>>()
                    .await
                    .unwrap();
                Some(status)
            }
            Err(_) => None,
        }
    }

    /// Remove unused images from docker
    ///
    /// # Arguments
    ///
    /// * `keep_if_created_before_time` - A time string indicating a duration since now.
    /// Images created before them will be deleted. Defaults to 1 hour ago.
    ///
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// docker.prune_images("10m"); // prune images more than 10 min old
    /// ```
    pub async fn prune_images(&self, keep_if_created_before_time: Option<&str>) {
        let mut filters = HashMap::new();
        filters.insert("until", vec![keep_if_created_before_time.unwrap_or("1h")]); // keep images created < until ago
        filters.insert("dangling", vec!["false"]); // remove all images that are not running

        let out = self
            .conn
            .prune_images(Some(PruneImagesOptions { filters }))
            .await
            .unwrap();

        info!(
            "Docker prune removed {} images, reclaimed {} bytes",
            out.images_deleted.unwrap_or_else(|| Vec::new()).len(),
            out.space_reclaimed.unwrap_or(0)
        );
    }

    /// Remove unused containers from docker
    ///
    /// # Arguments
    ///
    /// * `keep_if_created_before_time` - A time string indicating a duration since now.
    /// Containers created before them will be deleted. Defaults to 1 hour ago.
    ///
    /// # Examples
    ///
    /// ```
    /// let docker = DockerBroker::new();
    /// docker.prune_containers("10m"); // prune containers more than 10 min old
    /// ```
    pub async fn prune_containers(&self, keep_if_created_before_time: Option<&str>) {
        let mut filters = HashMap::new();
        filters.insert("until", vec![keep_if_created_before_time.unwrap_or("1h")]); // keep images created < until ago

        let out = self
            .conn
            .prune_containers(Some(PruneContainersOptions { filters }))
            .await
            .unwrap();

        info!(
            "Docker prune removed {} images, reclaimed {} bytes",
            out.containers_deleted.unwrap_or_else(|| Vec::new()).len(),
            out.space_reclaimed.unwrap_or(0)
        );
    }

    /// Removes unused containers and images from docker
    /// Uses `docker::DockerBroker::prune_containers` and `docker::DockerBroker::prune_images` default `keep_if_created_before_time`.
    pub async fn prune(&self) {
        self.prune_containers(None).await;
        self.prune_images(None).await;
    }
}

/// Struct to capture the results from a build operation
#[derive(Debug)]
pub struct DockerImageBuildResult {
    pub log: Vec<String>,
    pub image_id: String,
}
/// Struct to capture the results from a container status check
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ContainerStatus {
    pub state: bollard::models::ContainerStateStatusEnum,
    pub size: i32,
    pub mem_mb: i32,
    pub mem_max_mb: i32,
    pub cpu_usage: f32,
}

#[cfg(test)]
mod test {

    use super::*;

    #[tokio::test]
    async fn docker_broker_can_connect_to_socket() {
        let broker = DockerBroker::new().await;

        assert!(broker.is_some());

        if let Some(b) = broker {
            b.get_running_containers().await;
            assert!(true);
        }
    }

    #[tokio::test]
    async fn docker_prune_should_never_fail() {
        let broker = DockerBroker::new().await;

        assert!(broker.is_some());

        if let Some(b) = broker {
            b.prune().await;
            assert!(true);
        }
    }
}
