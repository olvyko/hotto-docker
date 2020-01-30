use dockerust::*;

#[tokio::test]
async fn test_generic_image() {
    let image = GenericImage::new("postgres:11-alpine")
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "db")
        .with_env_var("POSTGRES_USER", "user")
        .with_env_var("POSTGRES_PASSWORD", "pass");

    let container = Docker::run(image).create_container().await;
    Docker::logs(container.id()).print_stdout().await;
}
