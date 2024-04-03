use anyhow::{Ok, Result};
use axum::{extract::Query, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs::File, sync::Arc};
use vibrato::{
    dictionary::{LexType, WordIdx},
    Dictionary, Tokenizer,
};

#[derive(Deserialize)]
struct TokenizeReq {
    text: String,
}

#[derive(Deserialize)]
struct FeatureReq {
    id: u32,
    lex_type: Option<u8>,
}

#[derive(Serialize)]
struct Token {
    id: u32,
    surface: String,
    lex_type: u8,
    range_byte: [usize; 2],
    range_char: [usize; 2],
}

impl<'a, 'b> From<vibrato::token::Token<'a, 'b>> for Token {
    fn from(value: vibrato::token::Token) -> Self {
        Self {
            id: value.word_idx().word_id,
            surface: value.surface().to_string(),
            lex_type: match value.lex_type() {
                LexType::Unknown => 0,
                LexType::System => 1,
                LexType::User => 2,
            },
            range_byte: [value.range_byte().start, value.range_byte().end],
            range_char: [value.range_char().start, value.range_char().end],
        }
    }
}

fn tokenize(tokenizer: Arc<Tokenizer>, text: String) -> Json<Vec<Token>> {
    let mut worker = tokenizer.new_worker();
    worker.reset_sentence(text);
    worker.tokenize();
    let tokens = worker.token_iter().map(Into::into).collect::<Vec<_>>();

    Json(tokens)
}

#[tokio::main]
async fn main() -> Result<()> {
    let dict_path = std::env::var("DICT_PATH")?;
    let reader = zstd::Decoder::new(File::open(&dict_path)?)?;
    let dict = Dictionary::read(reader)?;
    let tokenizer = Arc::new(Tokenizer::new(dict));
    let tokenizer2 = tokenizer.clone();
    let word_tokenizer = tokenizer.clone();

    let router = Router::new()
        .route(
            "/tokenize",
            get(|req: Query<TokenizeReq>| async move { tokenize(tokenizer, req.text.clone()) })
                .post(
                    |req: Json<TokenizeReq>| async move { tokenize(tokenizer2, req.text.clone()) },
                ),
        )
        .route(
            "/feature",
            get(|req: Query<FeatureReq>| async move {
                let dict = word_tokenizer.dictionary();
                let feature = dict.word_feature(WordIdx {
                    word_id: req.id,
                    lex_type: match req.lex_type {
                        Some(1) | None => LexType::System,
                        Some(2) => LexType::User,
                        _ => LexType::Unknown,
                    },
                });

                Json(json!({
                    "feature": feature
                }))
            }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:5000").await?;
    axum::serve(listener, router.into_make_service()).await?;

    Ok(())
}
