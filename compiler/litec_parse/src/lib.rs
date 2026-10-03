mod lexer;
pub mod parser;

pub use lexer::tokenize;

#[cfg(test)]
mod tests;