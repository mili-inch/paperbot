use anyhow::{Error, Result};
use dotenvy::dotenv;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use tokio::sync::RwLock;

use serenity::async_trait;
use serenity::builder::CreateCommand;
use serenity::builder::CreateEmbed;
use serenity::builder::CreateInteractionResponse;
use serenity::builder::CreateInteractionResponseMessage;
use serenity::builder::CreateMessage;
use serenity::builder::CreateThread;
use serenity::client::ClientBuilder;
use serenity::client::Context;
use serenity::client::EventHandler;
use serenity::model::application::Command;
use serenity::model::application::Interaction;
use serenity::model::channel::Message;
use serenity::model::gateway::GatewayIntents;
use serenity::model::gateway::Ready;
use serenity::model::id::ChannelId;

mod get_paper_info;

fn load_enabled_channels() -> HashSet<ChannelId> {
    let path = "enabled_channels.json";
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(vec) = serde_json::from_str::<Vec<u64>>(&content) {
            return vec.into_iter().map(ChannelId::new).collect();
        }
    }
    HashSet::new()
}

fn save_enabled_channels(channels: &HashSet<ChannelId>) {
    let path = "enabled_channels.json";
    let vec: Vec<u64> = channels.iter().map(|cid| cid.get()).collect();
    if let Ok(json) = serde_json::to_string(&vec) {
        let _ = fs::write(path, json);
    }
}

static ENABLED_CHANNELS: Lazy<RwLock<HashSet<ChannelId>>> =
    Lazy::new(|| RwLock::new(load_enabled_channels()));

struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };
        let channel_id = command.channel_id;
        let mut enabled_channels = ENABLED_CHANNELS.write().await;
        if command.data.name == "enable" {
            enabled_channels.insert(channel_id);
            save_enabled_channels(&*enabled_channels);
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("Enabled the bot in this channel"),
            );
            command
                .create_response(&ctx.http, response)
                .await
                .expect("Failed to create response");
        } else if command.data.name == "disable" {
            enabled_channels.remove(&channel_id);
            save_enabled_channels(&*enabled_channels);
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("Disabled the bot in this channel"),
            );
            command
                .create_response(&ctx.http, response)
                .await
                .expect("Failed to create response");
        }
    }
    async fn ready(&self, ctx: Context, _ready: Ready) {
        Command::create_global_command(
            &ctx.http,
            CreateCommand::new("enable").description("Enable the bot in this channel"),
        )
        .await
        .expect("Failed to create command");
        Command::create_global_command(
            &ctx.http,
            CreateCommand::new("disable").description("Disable the bot in this channel"),
        )
        .await
        .expect("Failed to create command");
        println!("Bot is ready");
    }
    async fn message(&self, ctx: Context, msg: Message) {
        let enabled_channels = ENABLED_CHANNELS.read().await;
        if !enabled_channels.contains(&msg.channel_id) {
            return;
        }
        let re = Regex::new(r"\d{4}\.\d{4,5}").unwrap();
        if let Some(arxiv_id) = re.find(&msg.content) {
            println!("Found arxiv id: {}", arxiv_id.as_str());
            let paper = get_paper_info::get_paper_info(arxiv_id.as_str()).await;
            match paper {
                Ok(paper) => {
                    let embed = CreateEmbed::default()
                        .title(&paper.title)
                        .field("Published", paper.published.to_string(), false)
                        .field("Semantic Scholar", paper.semantic_scholar_url, false)
                        .field("Connected Papers", paper.connected_papers_url, false)
                        .field(
                            "Authors",
                            paper
                                .authors
                                .iter()
                                .map(|author| format!("[{}]({})", author.name, author.author_url))
                                .collect::<Vec<String>>()
                                .join(", "),
                            false,
                        );
                    let thread_channel = msg
                        .channel_id
                        .create_thread_from_message(
                            &ctx.http,
                            msg.id,
                            CreateThread::new(paper.title.chars().take(100).collect::<String>()),
                        )
                        .await
                        .expect("Failed to create thread");
                    thread_channel
                        .send_message(&ctx.http, CreateMessage::new().embed(embed))
                        .await
                        .expect("Failed to send message");
                    thread_channel
                        .send_message(
                            &ctx.http,
                            CreateMessage::new()
                                .content(format!("Summary: ```{}```", paper.summary)),
                        )
                        .await
                        .expect("Failed to send message");
                    thread_channel
                        .send_message(
                            &ctx.http,
                            CreateMessage::new()
                                .content(format!("Translated: ```{}```", paper.translated_summary)),
                        )
                        .await
                        .expect("Failed to send message");
                }
                Err(e) => {
                    msg.reply(&ctx.http, format!("Failed to get paper info: {}", e))
                        .await
                        .expect("Failed to send message");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN")?;
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    ClientBuilder::new(token, intents)
        .event_handler(Handler)
        .await?
        .start()
        .await?;

    Ok(())
}
