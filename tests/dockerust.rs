use dockerust::*;

#[test]
fn test_generic_image() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init()
        .unwrap();

    let image = GenericImage::new("postgres:11-alpine")
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
            20,
        ))
        .with_env_var("POSTGRES_DB", "db")
        .with_env_var("POSTGRES_USER", "user")
        .with_env_var("POSTGRES_PASSWORD", "pass");

    let container = DockerContainer::new(image).unwrap();
    container.run_background_logs_stderr();
    std::thread::sleep(std::time::Duration::from_secs(10));
}
