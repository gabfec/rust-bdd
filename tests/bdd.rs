use cucumber::World;
use my_bdd::steps::MyWorld;

#[tokio::test]
async fn run_bdd() {
    MyWorld::cucumber()
        .with_default_cli() // This ensures proper CLI handling
        .run("tests/features/ping_pong.feature")
        .await;
}
