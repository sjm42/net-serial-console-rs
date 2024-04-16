// event.rs

use bytes::BytesMut;
use tokio::io;
use tokio_util::codec::Decoder;

use crate::*;

const LINE_WRAP: usize = 80;

#[derive(Clone, Debug, Default)]
pub struct EventCodec {
    next_index: usize,
    id: u64,
}

impl EventCodec {
    pub fn new() -> EventCodec {
        EventCodec {
            next_index: 0,
            id: 0,
        }
    }
}

impl Decoder for EventCodec {
    type Item = String;
    type Error = tokio::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<String>, io::Error> {
        let read_to = buf.len();
        let newline_offset = buf[self.next_index..read_to]
            .iter()
            .position(|b| *b == b'\n');

        match newline_offset {
            Some(offset) => {
                let newline_index = self.next_index + offset;
                let bline;
                let line_out;
                if newline_index < LINE_WRAP {
                    bline = buf.split_to(newline_index + 1);
                    let line = &bline[..bline.len() - 1];
                    line_out = without_carriage_return(line);
                    self.next_index = 0;
                    if line_out.is_empty() {
                        return Ok(None);
                    }
                } else {
                    bline = buf.split_to(LINE_WRAP);
                    line_out = &bline[..LINE_WRAP];
                    self.next_index = 0;
                }
                self.id += 1;
                let s = &String::from_utf8_lossy(line_out);
                let s_printable = s.replace(|c: char| !is_printable_ascii(c), "_");
                let ev = format!(
                    "retry: 999999\r\nid: {id}\r\ndata: {s_printable}\r\n\r\n",
                    id = self.id
                );
                debug!("id {id} data: {s_printable}", id = self.id);
                Ok(Some(ev))
            }
            None => {
                self.next_index = read_to;
                Ok(None)
            }
        }
    }
}

fn without_carriage_return(s: &[u8]) -> &[u8] {
    if let Some(&b'\r') = s.last() {
        &s[..s.len() - 1]
    } else {
        s
    }
}

fn is_printable_ascii(c: char) -> bool {
    let cu = c as u32;
    cu > 0x1F && cu < 0x7F
}
// EOF
