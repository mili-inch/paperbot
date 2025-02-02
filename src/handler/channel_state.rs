use serde_json;
use serenity::model::id::ChannelId;
use std::collections::HashSet;
use std::fs;
use tokio::sync::RwLock;

pub struct ChannelState {
    pub channels: RwLock<HashSet<ChannelId>>,
}

impl ChannelState {
    pub fn new() -> Self {
        let channels = {
            let path = "enabled_channels.json";
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(vec) = serde_json::from_str::<Vec<u64>>(&content) {
                    vec.into_iter().map(ChannelId::new).collect()
                } else {
                    HashSet::new()
                }
            } else {
                HashSet::new()
            }
        };
        Self {
            channels: RwLock::new(channels),
        }
    }

    pub async fn add(&self, channel_id: ChannelId) {
        let mut channels = self.channels.write().await;
        channels.insert(channel_id);
    }

    pub async fn remove(&self, channel_id: ChannelId) {
        let mut channels = self.channels.write().await;
        channels.remove(&channel_id);
    }

    pub async fn contains(&self, channel_id: ChannelId) -> bool {
        let channels = self.channels.read().await;
        channels.contains(&channel_id)
    }

    pub async fn save(&self) {
        let channels = self.channels.read().await;
        let vec: Vec<u64> = channels.iter().map(|cid| cid.get()).collect();
        if let Ok(json) = serde_json::to_string(&vec) {
            let _ = fs::write("enabled_channels.json", json);
        }
    }
}
