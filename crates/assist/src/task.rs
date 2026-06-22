//! Request/answer shapes shared between app code and future workers.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Explain(String),
    Compose(String),
    Search(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Text(String),
    Command(String),
}
