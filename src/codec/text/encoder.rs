use std::io::{BufWriter, Result, Write};

use crate::{Encode, Event, Source};

pub struct TextEncoder<W: Write> {
    writer: BufWriter<W>,
}

impl<W: Write> TextEncoder<W> {
    pub fn new(writer: W, sources: &[Source]) -> Result<Self> {
        let mut enc = Self {
            writer: BufWriter::new(writer),
        };
        enc.write_header()?;
        enc.write_metadata(sources)?;
        Ok(enc)
    }

    fn write_header(&mut self) -> Result<()> {
        writeln!(self.writer, "HDR:format=accemic//ctxp-txt,ver=1")?;
        Ok(())
    }

    fn write_metadata(&mut self, sources: &[Source]) -> Result<()> {
        write!(self.writer, "META:")?;
        for (i, source) in sources.iter().enumerate() {
            if i > 0 {
                write!(self.writer, ",")?;
            }
            let escaped = source.name.replace('\\', "\\\\").replace('"', "\\\"");
            write!(self.writer, "#{}=\"{}\"", source.id, &escaped)?;
        }
        writeln!(self.writer)?;
        Ok(())
    }
}

impl<W: Write> Encode for TextEncoder<W> {
    fn write_event(&mut self, event: &Event) -> Result<()> {
        writeln!(self.writer, "{}", event)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::{Encode, Source, TextEncoder};

    #[test]
    fn write_meta_matches_spec_example() {
        let sources = vec![
            Source {
                id: 0,
                name: "CPU0".into(),
            },
            Source {
                id: 1,
                name: "CPU1".into(),
            },
            Source {
                id: 2,
                name: "CPU2".into(),
            },
            Source {
                id: 3,
                name: r#"CPU\"3""#.into(),
            },
        ];

        let mut enc = TextEncoder::new(Vec::new(), &sources).unwrap();
        enc.flush().unwrap();

        let output = String::from_utf8(enc.writer.into_inner().unwrap()).unwrap();
        let hdr_line = output.lines().nth(0).unwrap(); // line 0 is HDR
        let meta_line = output.lines().nth(1).unwrap(); // line 0 is HDR

        assert_eq!(hdr_line, r#"HDR:format=accemic//ctxp-txt,ver=1"#);
        assert_eq!(
            meta_line,
            r#"META:#0="CPU0",#1="CPU1",#2="CPU2",#3="CPU\\\"3\"""#
        );
    }
}
