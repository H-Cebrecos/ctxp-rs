use std::io::{BufRead, BufReader, Read};

use crate::{Decode, Event, Source, error};

pub struct TextDecoder<R: Read> {
    reader: BufReader<R>,
    sources: Vec<Source>,
}

impl<R: Read> TextDecoder<R> {
    pub fn new(reader: R) -> error::Result<Self> {
        let mut dec = Self {
            reader: BufReader::new(reader),
            sources: Vec::new(),
        };
        dec.read_header()?;
        dec.read_metadata()?;
        Ok(dec)
    }

    fn read_header(&mut self) -> error::Result<()> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        if line.trim_end() != "HDR:format=accemic//ctxp-txt,ver=1" {
            return Err(error::Error::Parse(format!(
                "invalid or unsupported header: '{}'",
                line.trim_end()
            )));
        }
        Ok(())
    }

    fn read_metadata(&mut self) -> error::Result<()> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;

        let line = line.trim_end();
        let entries = line
            .strip_prefix("META:")
            .ok_or_else(|| error::Error::Parse("expected META section".into()))?;

        self.sources = parse_meta_entries(entries)?;
        Ok(())
    }
}

impl<R: Read> Iterator for TextDecoder<R> {
    type Item = error::Result<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None,                                   // clean EOF
            Ok(_) => Some(line.trim_end().parse::<Event>()), // FromStr does the work
            Err(e) => Some(Err(e.into())),
        }
    }
}

impl<R: Read> Decode for TextDecoder<R> {
    fn sources(&self) -> &[Source] {
        &self.sources
    }
}

fn parse_meta_entries(s: &str) -> error::Result<Vec<Source>> {
    let mut sources = Vec::new();
    let mut chars = s.chars().peekable();

    loop {
        // expect '#'
        match chars.next() {
            Some('#') => {}
            None => break,
            Some(c) => {
                return Err(error::Error::Parse(format!("expected '#', got '{}'", c)));
            }
        }

        // parse decimal source id up to '='
        let mut id_str = String::new();
        loop {
            match chars.next() {
                Some('=') => break,
                Some(c) => id_str.push(c),
                None => {
                    return Err(error::Error::Parse("unexpected end in source id".into()));
                }
            }
        }
        let id = id_str
            .parse::<u8>()
            .map_err(|_| error::Error::Parse(format!("invalid source id: '{}'", id_str)))?;

        // expect opening quote
        match chars.next() {
            Some('"') => {}
            _ => return Err(error::Error::Parse("expected '\"' after '='".into())),
        }

        // parse name with unescape, up to closing unescaped quote
        let mut name = String::new();
        loop {
            match chars.next() {
                Some('\\') => match chars.next() {
                    Some('"') => name.push('"'),
                    Some('\\') => name.push('\\'),
                    Some(c) => {
                        return Err(error::Error::Parse(format!(
                            "invalid escape sequence: '\\{}'",
                            c
                        )));
                    }
                    None => {
                        return Err(error::Error::Parse("unexpected end in escape".into()));
                    }
                },
                Some('"') => break, // closing quote
                Some(c) => name.push(c),
                None => {
                    return Err(error::Error::Parse("unterminated source name".into()));
                }
            }
        }

        sources.push(Source { id, name });

        // expect ',' separator or end of string
        match chars.peek() {
            Some(',') => {
                chars.next();
            }
            _ => break,
        }
    }

    Ok(sources)
}
