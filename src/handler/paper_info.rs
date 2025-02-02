use anyhow::{Error, Result};
use chrono::NaiveDateTime;
use reqwest::Client;
use serde::Deserialize;
use translators::{GoogleTranslator, Translator};
use urlencoding::encode;

#[derive(Debug, Deserialize)]
struct ArxivEntry {
    title: String,
    summary: String,
    published: String,
}

#[derive(Debug, Deserialize)]
struct ArxivResponse {
    #[serde(rename = "entry")]
    entry: ArxivEntry,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarAuthor {
    name: String,

    #[serde(rename = "authorId")]
    author_id: Option<String>,
}

#[derive(Debug)]
pub struct Author {
    pub name: String,
    pub author_url: Option<String>,
}

#[derive(Debug)]
pub struct Paper {
    pub title: String,
    pub published: NaiveDateTime,
    pub summary: String,
    pub translated_summary: String,
    pub authors: Vec<Author>,
    pub semantic_scholar_url: String,
    pub connected_papers_url: String,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarResponse {
    #[serde(rename = "paperId")]
    paper_id: String,

    #[serde(rename = "authors")]
    authors: Vec<SemanticScholarAuthor>,
}

async fn fetch_arxiv_entry(arxiv_id: &str) -> Result<ArxivEntry, Error> {
    let url = format!("http://export.arxiv.org/api/query?id_list={}", arxiv_id);
    let response = Client::new().get(&url).send().await?.text().await?;
    let feed: ArxivResponse = serde_xml_rs::from_str(&response)?;
    Ok(feed.entry)
}

async fn fetch_semantic_scholar_response(arxiv_id: &str) -> Result<SemanticScholarResponse, Error> {
    let base_arxiv_url = "https://arxiv.org/abs/";
    let url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/url:{}?fields=authors",
        encode(&format!("{}{}", base_arxiv_url, arxiv_id))
    );
    let response = Client::new().get(&url).send().await?.text().await?;
    println!("{}", response);
    let response = serde_json::from_str(&response)?;

    Ok(response)
}

pub async fn get_paper_info(arxiv_id: &str) -> Result<Paper, Error> {
    let arxiv_entry = fetch_arxiv_entry(arxiv_id).await?;
    let semantic_scholar_response = fetch_semantic_scholar_response(arxiv_id).await?;
    let semantic_scholar_url = format!(
        "https://www.semanticscholar.org/paper/{}",
        semantic_scholar_response.paper_id
    );
    let connected_papers_url = format!(
        "https://www.connectedpapers.com/main/{}",
        semantic_scholar_response.paper_id
    );
    let translator = GoogleTranslator::default();
    let translated_summary = translator
        .translate_async(&arxiv_entry.summary, "", "ja")
        .await?;
    let authors = semantic_scholar_response
        .authors
        .iter()
        .map(|author| Author {
            name: author.name.clone(),
            author_url: author
                .author_id
                .clone()
                .map(|id| format!("https://www.semanticscholar.org/author/{}", id)),
        })
        .collect();
    let paper = Paper {
        title: arxiv_entry.title,
        published: NaiveDateTime::parse_from_str(&arxiv_entry.published, "%Y-%m-%dT%H:%M:%SZ")?,
        summary: arxiv_entry.summary,
        translated_summary: translated_summary,
        authors,
        semantic_scholar_url,
        connected_papers_url,
    };
    Ok(paper)
}
