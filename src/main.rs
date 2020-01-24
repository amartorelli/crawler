extern crate clap;
extern crate queues;
extern crate regex;
extern crate reqwest;
extern crate select;

use clap::{App, Arg};
use queues::*;
use regex::Regex;
use select::document::Document;
use select::predicate::Name;
use std::collections::HashSet;
use std::result::Result;
use url::Url;

struct Crawler {
    client: reqwest::Client,
    target: String,
    visited: HashSet<String>,
    queue: Queue<String>,
    base_url: Url,
}

impl Crawler {
    fn new(target: &String) -> Crawler {
        let client = reqwest::Client::new();
        let visited: HashSet<String> = HashSet::new();
        let queue: Queue<String> = queue![];
        let burl: Url;

        match Url::parse(target.as_str()) {
            Ok(base_url) => burl = base_url,
            Err(e) => panic!("{}", e),
        }

        Crawler {
            client: client,
            target: target.to_string(),
            visited: visited,
            queue: queue,
            base_url: burl,
        }
    }

    #[tokio::main]
    async fn fetch(&self) -> Result<String, reqwest::Error> {
        let body = self
            .client
            .get(self.target.as_str())
            .send()
            .await?
            .text()
            .await?;
        Ok(body)
    }

    fn run(&mut self) {
        match self
            .queue
            .add(self.convert_link_to_abs(self.base_url.as_str()))
        {
            Err(e) => println!("{}", e),
            _ => (),
        }
        while self.queue.size() > 0 {
            match self.fetch() {
                Ok(content) => match self.get_links(content) {
                    Ok(()) => println!("added links: {:#?}", self.queue),
                    Err(e) => println!("{}", e),
                },
                Err(e) => println!("{}", e),
            }
        }
    }

    fn convert_link_to_abs(&self, link: &str) -> String {
        let re = Regex::new(r"^http://.*").unwrap();
        if !re.is_match(link) {
            match self.base_url.join(link) {
                Ok(full_url) => return full_url.into_string(),
                Err(e) => {
                    println!("unable to convert link to abs: {}", e);
                    return String::new();
                }
            };
        } else {
            String::from(link)
        }
    }

    fn get_links(&mut self, content: String) -> Result<(), reqwest::Error> {
        Document::from(content.as_str())
            .find(Name("a"))
            .filter_map(|n| n.attr("href"))
            .for_each(|l| {
                let link = &String::from(l);
                let full_link = self.convert_link_to_abs(link);
                if !self.visited.contains(&full_link) {
                    match self.queue.add(full_link.to_string()) {
                        Err(e) => println!("{}", e),
                        _ => (),
                    }
                    self.visited.insert(full_link.to_string());
                }
            });
        Ok(())
    }
}

fn main() {
    let opts = App::new("Crawler")
        .about("Crawler crawls a website")
        .author("Alessio M.")
        .arg(
            Arg::with_name("target")
                .help("the target to crawl")
                .required(true)
                .short("t")
                .takes_value(true),
        );
    let matches = opts.get_matches();

    if let Some(t) = matches.value_of("target") {
        let mut crawler: Crawler = Crawler::new(&t.to_string());
        crawler.run();
    } else {
        println!("unable to parse target");
    }
}
