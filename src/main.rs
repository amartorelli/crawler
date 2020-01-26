#[macro_use]
extern crate lazy_static;
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
    fetch_any_domain: bool,
    domain_regex: Regex,
}

impl Crawler {
    fn new(target: &String, any_domain: bool) -> Crawler {
        let client = reqwest::Client::new();
        let visited: HashSet<String> = HashSet::new();
        let queue: Queue<String> = queue![];
        let burl: Url;

        match Url::parse(target.as_str()) {
            Ok(base_url) => burl = base_url,
            Err(e) => panic!("{}", e),
        }

        let mut rule = String::from("^");
        rule.push_str(burl.as_str());
        rule.push_str(".*");
        Crawler {
            client: client,
            target: target.to_string(),
            visited: visited,
            queue: queue,
            base_url: burl,
            fetch_any_domain: any_domain,
            domain_regex: Regex::new(rule.as_str()).unwrap(),
        }
    }

    #[tokio::main]
    async fn fetch(&self, target: &str) -> Result<String, reqwest::Error> {
        let body = self.client.get(target).send().await?.text().await?;
        Ok(body)
    }

    fn should_fetch(&self, link: &str) -> bool {
        if self.fetch_any_domain || self.domain_regex.is_match(link) {
            println!("Comparing {}/{:#?} should fetch", link, self.domain_regex);
            return true;
        }
        println!(
            "Comparing {}/{:#?} should not fetch",
            link, self.domain_regex
        );
        println!("Domain {} should not fetch", link);
        false
    }

    fn run(&mut self) {
        match self
            .queue
            .add(self.convert_link_to_abs(self.target.as_str()))
        {
            Err(e) => println!("{}", e),
            _ => (),
        }
        while self.queue.size() > 0 {
            match self.queue.remove() {
                Ok(link) => match self.fetch(link.as_str()) {
                    Ok(content) => match self.get_links(content) {
                        Ok(()) => println!("added links: {:#?}", self.queue),
                        Err(e) => println!("{}", e),
                    },
                    Err(e) => println!("{}", e),
                },
                Err(e) => println!("{}", e),
            }
        }
    }

    fn convert_link_to_abs(&self, link: &str) -> String {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^http://.*").unwrap();
        }
        if !RE.is_match(link) {
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
                    if self.should_fetch(&full_link) {
                        match self.queue.add(full_link.to_string()) {
                            Err(e) => println!("{}", e),
                            _ => (),
                        }
                        self.visited.insert(full_link.to_string());
                    }
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
        )
        .arg(Arg::with_name("any").help("fetch any domain").short("a"));
    let matches = opts.get_matches();

    let mut any_domain = false;
    if let Some(_a) = matches.value_of("any") {
        any_domain = true;
    }

    if let Some(t) = matches.value_of("target") {
        let mut crawler: Crawler = Crawler::new(&t.to_string(), any_domain);
        crawler.run();
    } else {
        println!("unable to parse target");
    }
}
