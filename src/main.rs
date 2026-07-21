use std::{
    io::{self, Write},
    process,
    str::FromStr,
};

use clap::Parser;
use color_eyre::eyre::{Context, OptionExt, Result, eyre};
use isolang::Language;
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Parser)]
struct Cli {
    /// The word to be translated
    word: String,
    /// The ISO-639 code for the language the word is translated from
    source_language: String,
    /// The ISO-639 code for the language the word is translated into
    target_language: String,
    /// Retrieve the summary of the translated page
    #[arg(short, long)]
    summary: bool,
    /// Open Wikipedia page for target language article
    #[arg(short, long)]
    open_in_browser: bool,
}

#[derive(Debug, Deserialize)]
pub struct LanguageEntry {
    pub code: String,
    pub name: String,
    pub key: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct Summary {
    pub extract: String,
}

#[derive(Debug, Deserialize)]
pub struct Page {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Pages {
    pub pages: Vec<Page>,
}

const USER_AGENT: &str = "wiki-term-translator";

fn run() -> Result<()> {
    #[cfg(debug_assertions)]
    color_eyre::install()?;

    let cli = Cli::parse();
    let client = Client::new();
    let source_language_code = Language::from_str(&cli.source_language)
        .map(|lang| lang.to_639_1().unwrap_or(lang.to_639_3()))
        .wrap_err("Invalid source language code")?;
    let target_language_code = Language::from_str(&cli.target_language)
        .map(|lang| lang.to_639_1().unwrap_or(lang.to_639_3()))
        .wrap_err("Invalid target language code")?;
    let pages = client
        .get(format!(
            "https://{}.wikipedia.org/w/rest.php/v1/search/title?q={}&limit=10",
            source_language_code, cli.word
        ))
        .header("User-Agent", USER_AGENT)
        .send()
        .wrap_err("Network error retrieving pages")?
        .json::<Pages>()
        .wrap_err("Couldn't parse network response of the Wikipedia pages correctly")?
        .pages;

    for (i, page) in pages.iter().enumerate().rev() {
        if let Some(desc) = &page.description
            && !desc.is_empty()
        {
            println!("{} - {} ({})", i + 1, page.title, desc);
        } else {
            println!("{} - {}", i + 1, page.title);
        }
    }

    // TODO: Improve prompt to be more describe (like yay)
    print!("> ");
    io::stdout().flush()?;

    let mut choice_buffer = String::new();

    io::stdin().read_line(&mut choice_buffer)?;

    let choice = pages
        .get(choice_buffer.trim_end().parse::<usize>()? - 1)
        .ok_or_eyre("Invalid index provided")?;

    let lang_entries = client
        .get(format!(
            "https://{}.wikipedia.org/w/rest.php/v1/page/{}/links/language",
            source_language_code, choice.key
        ))
        .header("User-Agent", USER_AGENT)
        .send()
        .wrap_err("Network error retrieving language entries")?
        .json::<Vec<LanguageEntry>>()
        .wrap_err("Couldn't parse network response of language entries correctly")?;

    if lang_entries.is_empty() {
        return Err(eyre!(
            "No entries were found for {} going from {} to {}",
            cli.word,
            cli.source_language,
            cli.target_language
        ));
    }

    for entry in lang_entries {
        if entry.code == *target_language_code || entry.code == cli.target_language {
            if cli.summary {
                let summary = client
                    .get(format!(
                        "https://{}.wikipedia.org/api/rest_v1/page/summary/{}",
                        target_language_code, entry.key
                    ))
                    .header("User-Agent", USER_AGENT)
                    .send()
                    .wrap_err("Network error retrieving article summary")?
                    .json::<Summary>()
                    .wrap_err("Couldn't parse network response of summary correctly")?
                    .extract;
                println!("{} - {}", entry.title, summary);
            } else {
                println!("{}", entry.title);
            }

            if cli.open_in_browser {
                webbrowser::open(&format!(
                    "https://{}.wikipedia.org/wiki/{}",
                    target_language_code, entry.key
                ))?;
            }
            return Ok(());
        }
    }

    Err(eyre!(
        "No entries were found for {} going from {} to {}",
        cli.word,
        cli.source_language,
        cli.target_language
    ))
}

fn main() {
    if let Err(err) = run() {
        if cfg!(debug_assertions) {
            eprintln!("Error: {err:?}");
        } else {
            eprintln!("Error: {err}");
        }
        process::exit(1);
    }
}
