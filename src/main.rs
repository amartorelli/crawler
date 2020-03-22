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
use std::sync::{Arc, Mutex};
use std::thread;
use url::Url;

struct Crawler {
    client: reqwest::Client,
    target: String,
    visited: Arc<Mutex<HashSet<String>>>,
    queue: Queue<String>,
    base_url: String,
    fetch_any_domain: bool,
}

impl Crawler {
    fn new(target: &String, any_domain: bool) -> Crawler {
        let client = reqwest::Client::new();
        let visited = Arc::new(Mutex::new(HashSet::new()));
        let queue = Queue::new();
        let burl: String;

        match url::Url::parse(target.as_str()) {
            Ok(url) => {
                if let Some(base_url) = url.host_str() {
                    burl = String::from(base_url);
                } else {
                    panic!("Unable to extract base url from target link {}", target);
                }
            }
            Err(e) => panic!("{}", e),
        }

        Crawler {
            client: client,
            target: target.to_string(),
            visited: visited,
            queue: queue,
            base_url: burl,
            fetch_any_domain: any_domain,
        }
    }

    #[tokio::main]
    async fn fetch(&self, target: &str) -> Result<String, reqwest::Error> {
        let body = self.client.get(target).send().await?.text().await?;
        Ok(body)
    }

    fn should_fetch(&self, link: &str) -> bool {
        if self.fetch_any_domain {
            return true;
        }
        self.is_same_domain(link)
    }

    fn is_same_domain(&self, link: &str) -> bool {
        match Url::parse(link) {
            Ok(url) => {
                if let Some(host) = url.host_str() {
                    println!("checking {} base url {} with {}", link, host, self.base_url);
                    return host == self.base_url;
                } else {
                    false
                }
            }
            Err(e) => {
                println!("{}", e);
                return false;
            }
        }
    }

    fn run(mut self) {
        {
            match self
                .queue
                .add(self.convert_link_to_abs(self.target.as_str()))
            {
                Err(e) => println!("{}", e),
                _ => (),
            }
        }

        loop {
            let l = self.queue.size();
            if l == 0 {
                println!("empty queue, exiting...");
                break;
            }

            match self.queue.remove() {
                Ok(link) => match self.fetch(link.as_str()) {
                    Ok(content) => match self.get_links(content) {
                        Ok(()) => println!("added new links"),
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
            static ref RE: Regex = Regex::new(r"^http[s]*://.*").unwrap();
        }
        if !RE.is_match(link) {
            let mut s = self.base_url.to_owned();
            s.push_str(link);
            return s;
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
                if !self.visited.lock().unwrap().contains(&full_link) {
                    if self.should_fetch(&full_link) {
                        match self.queue.add(full_link.to_string()) {
                            Err(e) => println!("{}", e),
                            _ => (),
                        }
                        self.visited.lock().unwrap().insert(full_link.to_string());
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
        .arg(Arg::with_name("any").help("fetch any domain").short("a"))
        .arg(
            Arg::with_name("workers")
                .help("number of workers to spin up")
                .short("w")
                .takes_value(true)
                .default_value("1"),
        );
    let matches = opts.get_matches();

    let mut any_domain = false;
    if let Some(_a) = matches.value_of("any") {
        any_domain = true;
    }

    let mut n_workers = 1;
    if let Some(w) = matches.value_of("workers") {
        match w.parse::<u8>() {
            Ok(v) => n_workers = v,
            Err(_) => println!("unable to parse workers, defaulting to {}", n_workers),
        }
    }

    if let Some(t) = matches.value_of("target") {
        let visited: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let mut threads = vec![];
        for i in 0..n_workers {
            println!("spawning worker {}", i);
            let crawler: Crawler = Crawler::new(&t.to_string(), any_domain);
            threads.push(thread::spawn(move || {
                crawler.run();
            }));
        }

        for t in threads {
            let _ = t.join().unwrap();
        }
    } else {
        println!("unable to parse target");
    }
}
