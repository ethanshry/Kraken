use crate::docker::DockerBroker;
use crate::file_utils::copy_dockerfile_to_dir;
use crate::git_utils::clone_remote_branch;
use crate::model::ApplicationStatus;
use crate::rabbit::{
    deployment_message::DeploymentMessage, QueueLabel, RabbitBroker, RabbitMessage,
};
use log::{error, info};
use uuid::Uuid;

pub async fn handle_deployment(
    system_id: &str,
    broker: &RabbitBroker,
    git_uri: &str,
) -> Result<(), ()> {
    let container_guid = Uuid::new_v4().to_hyphenated().to_string();

    let publisher = broker.get_channel().await;

    let mut msg = DeploymentMessage::new(&system_id, &container_guid);

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    // TODO make function to execute a thing in a tmp dir which auto-cleans itself
    let tmp_dir_path = "tmp/process";

    info!("Creating Container {}", &container_guid);

    msg.update_message(
        ApplicationStatus::RETRIEVING,
        &format!("Retrieving Application Data from {}", git_uri),
    );

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    info!("Retrieving git repository for container from {}", git_uri);
    clone_remote_branch(git_uri, "master", tmp_dir_path)
        .wait()
        .unwrap();

    // Inject the proper dockerfile into the project
    // TODO read configuration information from toml file

    let dockerfile_name = "python36.dockerfile";

    copy_dockerfile_to_dir(dockerfile_name, tmp_dir_path);

    info!("module path is {}", module_path!());

    msg.update_message(ApplicationStatus::BUILDING, "");

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    let docker = DockerBroker::new().await;
    if let Some(docker) = docker {
        let res = docker.build_image(tmp_dir_path).await;
        if let Ok(r) = res {
            info!("----- Docker Build Results for {} -----", r.image_id);
            info!("{:?}", r.log);
            msg.update_message(ApplicationStatus::BUILDING, &r.log.join("\n"));
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            msg.update_message(
                ApplicationStatus::BUILDING,
                &format!(
                    "docker image created for {} with id {}",
                    &git_uri,
                    &r.log.join("\n")
                ),
            );
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            // Image build went ok, now deploy the container
            msg.update_message(ApplicationStatus::DEPLOYING, "");
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            let ids = docker.start_container(&r.image_id, 9000).await;

            if let Ok(id) = ids {
                info!("Docker container started for {} with id {}", git_uri, id);
                msg.update_message(ApplicationStatus::DEPLOYED, &format!("{}", id));
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            } else {
                msg.update_message(ApplicationStatus::ERRORED, "Error in deployment");
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
                return Err(());
            }
        } else {
            msg.update_message(ApplicationStatus::ERRORED, "Error in build process");
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            error!("Failed to build docker image for {}", &git_uri);
            return Err(());
        }
    }

    Ok(())
}
