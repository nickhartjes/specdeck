//! Terminal records as a window receives them, in the app over a binary channel and in the web
//! viewer from WebAssembly: a sequence of `1, len (u32 BE), bytes` for output and
//! `2, cols (u16 BE), rows (u16 BE)` for a resize. Binary keeps a long replay cheap; the UI
//! decodes it in `app/ui/src/terminal/stream.ts`.

use crate::frames::Record;

pub fn encode_records(records: &[Record]) -> Vec<u8> {
    let size = records
        .iter()
        .map(|record| match record {
            Record::Output { bytes, .. } => 5 + bytes.len(),
            Record::Resize { .. } => 5,
        })
        .sum();
    let mut out = Vec::with_capacity(size);
    for record in records {
        match record {
            Record::Output { bytes, .. } => {
                out.push(1);
                out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                out.extend_from_slice(bytes);
            }
            Record::Resize { cols, rows, .. } => {
                out.push(2);
                out.extend_from_slice(&cols.to_be_bytes());
                out.extend_from_slice(&rows.to_be_bytes());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_output_and_resizes() {
        let records = [
            Record::Output { seq: 0, bytes: b"hi".to_vec() },
            Record::Resize { seq: 1, cols: 100, rows: 30 },
        ];
        assert_eq!(encode_records(&records), [1, 0, 0, 0, 2, b'h', b'i', 2, 0, 100, 0, 30]);
    }
}
