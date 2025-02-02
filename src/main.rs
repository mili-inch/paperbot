use anyhow::{Error, Result};
use dotenvy::dotenv;

use serenity::client::ClientBuilder;
use serenity::model::gateway::GatewayIntents;

mod handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN")?;
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let handler = handler::Handler::new();

    let mut client = ClientBuilder::new(token, intents)
        .event_handler(handler)
        .await?;

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }

    Ok(())
}
