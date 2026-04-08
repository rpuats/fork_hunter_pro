use std::sync::Arc;
use reqwest::Client;

use parsers::parser_factory::ParserFactory;

#[test]
fn factory_builds_parsers() {
    // Simple in-repo test to ensure factory can be constructed
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);
    let parsers = factory.get_all();
    assert!(!parsers.is_empty(), "ParserFactory should provide at least one parser");
}
