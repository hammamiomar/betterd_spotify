use std::{env, sync::Arc};

use neo4rs::{ConfigBuilder, Graph};
use dotenvy::dotenv;


pub async fn connect() -> Arc<Graph>{
    dotenv().ok();

    let uri = env::var("NEO4J_URI")
        .expect("NEO4J URI NOT SET");
    let user = env::var("NEO4J_USER")
        .expect("NEO4J USER NOT SET");
    let pass = env::var("NEO4J_PASS")
        .expect("NEO4J PASS NOT SET");

    let config = ConfigBuilder::default()
        .uri(uri)
        .user(&user)
        .password(&pass)
        .max_connections(50)
        .build()
        .unwrap();
    Arc::new(Graph::connect(config).await.unwrap())
}

