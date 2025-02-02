use regex::Regex;
use serenity::async_trait;
use serenity::builder::{
    CreateCommand, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
    CreateMessage, CreateThread, EditChannel,
};
use serenity::client::{Context, EventHandler};
use serenity::model::application::{Command, Interaction};
use serenity::model::channel::{GuildChannel, Message};
use serenity::model::gateway::Ready;

mod channel_state;
mod paper_info;

pub struct Handler {
    enabled_channels: channel_state::ChannelState,
    re: Regex,
}
impl Handler {
    pub fn new() -> Self {
        let enabled_channels = channel_state::ChannelState::new();
        let re = Regex::new(r"\d{4}\.\d{4,5}").unwrap();
        Self {
            enabled_channels,
            re,
        }
    }

    // New helper method to build the embed from paper info.
    fn build_embed(&self, paper: &paper_info::Paper) -> CreateEmbed {
        serenity::builder::CreateEmbed::default()
            .title(&paper.title)
            .field("Published", paper.published.to_string(), false)
            .field(
                "Semantic Scholar",
                paper.semantic_scholar_url.clone(),
                false,
            )
            .field(
                "Connected Papers",
                paper.connected_papers_url.clone(),
                false,
            )
            .field(
                "Authors",
                paper
                    .authors
                    .iter()
                    .map(|author| {
                        if let Some(author_url) = &author.author_url {
                            format!("[{}]({})", author.name, author_url)
                        } else {
                            author.name.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                false,
            )
    }

    // New helper method to send embed, summary, and translated summary,
    // using a send function provided as a closure.
    async fn send_paper_info_messages<F, Fut>(
        &self,
        http: &serenity::http::Http,
        send: F,
        paper: &paper_info::Paper,
    ) where
        F: Fn(CreateMessage) -> Fut,
        Fut: std::future::Future<Output = Result<Message, serenity::Error>> + Send,
    {
        let embed = self.build_embed(paper);
        send(CreateMessage::new().embed(embed))
            .await
            .expect("Failed to send embed");
        send(CreateMessage::new().content(format!("**Summary:** ```{}```", paper.summary)))
            .await
            .expect("Failed to send summary");
        send(CreateMessage::new().content(format!(
            "**Translated:** ```{}```",
            paper.translated_summary
        )))
        .await
        .expect("Failed to send translated summary");
    }
}
#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };
        if command.data.name == "enable" {
            self.enabled_channels.add(command.channel_id).await;
            self.enabled_channels.save().await;
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("Enabled the bot in this channel"),
            );
            command
                .create_response(&ctx.http, response)
                .await
                .expect("response should be sent");
        }
        if command.data.name == "disable" {
            self.enabled_channels.remove(command.channel_id).await;
            self.enabled_channels.save().await;
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("Disabled the bot in this channel"),
            );
            command
                .create_response(&ctx.http, response)
                .await
                .expect("response should be sent");
        }
    }
    async fn ready(&self, ctx: Context, _ready: Ready) {
        Command::create_global_command(
            &ctx.http,
            CreateCommand::new("enable").description("Enable the bot in this channel"),
        )
        .await
        .expect("command should be created");
        Command::create_global_command(
            &ctx.http,
            CreateCommand::new("disable").description("Disable the bot in this channel"),
        )
        .await
        .expect("command should be created");
        println!("Bot is ready");
    }
    async fn message(&self, ctx: Context, msg: Message) {
        let is_channel_enabled = self.enabled_channels.contains(msg.channel_id).await;
        if !is_channel_enabled {
            return;
        }
        let find_arxiv_id = self.re.find(&msg.content);
        if find_arxiv_id.is_none() {
            return;
        }
        let arxiv_id = find_arxiv_id.unwrap().as_str();
        println!("Found arxiv id: {}", arxiv_id);
        let paper = paper_info::get_paper_info(arxiv_id).await;
        if paper.is_err() {
            eprintln!("Failed to get paper info: {:?}", paper.err());
            return;
        }
        let paper = paper.unwrap();
        let thread_channel = msg
            .channel_id
            .create_thread_from_message(
                &ctx.http,
                msg.id,
                CreateThread::new(paper.title.chars().take(100).collect::<String>()),
            )
            .await
            .expect("Failed to create thread");
        // Use helper method instead of duplicating send_message calls.
        self.send_paper_info_messages(
            &ctx.http,
            |m| Box::pin(thread_channel.send_message(&ctx.http, m)),
            &paper,
        )
        .await;
    }
    async fn thread_create(&self, ctx: Context, mut thread: GuildChannel) {
        if thread.parent_id.is_none() {
            return;
        }
        let parent_id = thread.parent_id.unwrap();
        let is_channel_enabled = self.enabled_channels.contains(parent_id).await;
        if !is_channel_enabled {
            return;
        }
        let find = self.re.find(&thread.name);
        if find.is_none() {
            return;
        }
        let arxiv_id = find.unwrap().as_str();
        println!("Found arxiv id: {}", arxiv_id);
        let paper = paper_info::get_paper_info(arxiv_id).await;
        if paper.is_err() {
            eprintln!("Failed to get paper info: {:?}", paper.err());
            return;
        }
        let paper = paper.unwrap();
        let edit = EditChannel::new().name(paper.title.chars().take(100).collect::<String>());
        thread
            .edit(&ctx.http, edit)
            .await
            .expect("Failed to edit thread");
        // Use helper method to send details.
        self.send_paper_info_messages(
            &ctx.http,
            |m| Box::pin(thread.send_message(&ctx.http, m)),
            &paper,
        )
        .await;
    }
}
