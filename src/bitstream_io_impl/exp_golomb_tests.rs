use std::io::ErrorKind;

use super::bitstream_io_reader::BsIoSliceReader;
use super::bitstream_io_writer::BitstreamIoWriter;

// These vectors spell out the wire bits independently of the production writer.
fn bits(wire: &str) -> Vec<u8> {
    let mut bytes = vec![0; wire.len().div_ceil(8)];
    for (index, bit) in wire.bytes().enumerate() {
        match bit {
            b'1' => bytes[index / 8] |= 1 << (7 - index % 8),
            b'0' => (),
            _ => panic!("invalid test vector"),
        }
    }
    bytes
}

fn code(prefix_zeroes: usize, suffix: &str) -> String {
    assert_eq!(prefix_zeroes, suffix.len());
    format!("{}1{suffix}", "0".repeat(prefix_zeroes))
}

fn ue_vectors() -> Vec<(u64, String)> {
    vec![
        (0, "1".into()),
        (1, "010".into()),
        (2, "011".into()),
        (3, "00100".into()),
        (4, "00101".into()),
        (15, "000010000".into()),
        (16, "000010001".into()),
        (
            u64::MAX - 2,
            code(
                63,
                "111111111111111111111111111111111111111111111111111111111111110",
            ),
        ),
        (
            u64::MAX - 1,
            code(
                63,
                "111111111111111111111111111111111111111111111111111111111111111",
            ),
        ),
        (
            u64::MAX,
            code(
                64,
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
        ),
    ]
}

fn se_vectors() -> Vec<(i64, String)> {
    vec![
        (0, "1".into()),
        (1, "010".into()),
        (-1, "011".into()),
        (2, "00100".into()),
        (-2, "00101".into()),
        (
            9_007_199_254_740_991,
            code(53, "11111111111111111111111111111111111111111111111111110"),
        ),
        (
            -9_007_199_254_740_991,
            code(53, "11111111111111111111111111111111111111111111111111111"),
        ),
        (
            9_007_199_254_740_992,
            code(54, "000000000000000000000000000000000000000000000000000000"),
        ),
        (
            -9_007_199_254_740_992,
            code(54, "000000000000000000000000000000000000000000000000000001"),
        ),
        (
            9_007_199_254_740_993,
            code(54, "000000000000000000000000000000000000000000000000000010"),
        ),
        (
            -9_007_199_254_740_993,
            code(54, "000000000000000000000000000000000000000000000000000011"),
        ),
        (
            i64::MAX,
            code(
                63,
                "111111111111111111111111111111111111111111111111111111111111110",
            ),
        ),
        (
            -i64::MAX,
            code(
                63,
                "111111111111111111111111111111111111111111111111111111111111111",
            ),
        ),
    ]
}

#[test]
fn reads_manual_unsigned_vectors() {
    for (expected, wire) in ue_vectors() {
        let bytes = bits(&wire);
        let mut reader = BsIoSliceReader::from_slice(&bytes);
        assert_eq!(reader.read_ue().unwrap(), expected, "{wire}");
        assert_eq!(reader.position_in_bits().unwrap(), wire.len() as u64);
    }
}

#[test]
fn reads_manual_signed_vectors_without_precision_loss() {
    for (expected, wire) in se_vectors() {
        let bytes = bits(&wire);
        let mut reader = BsIoSliceReader::from_slice(&bytes);
        assert_eq!(reader.read_se().unwrap(), expected, "{wire}");
        assert_eq!(reader.position_in_bits().unwrap(), wire.len() as u64);
    }
}

#[test]
fn rejects_unsigned_overflow_and_stops_at_the_65th_prefix_zero() {
    for suffix in [
        "0000000000000000000000000000000000000000000000000000000000000001",
        "1111111111111111111111111111111111111111111111111111111111111111",
    ] {
        let bytes = bits(&code(64, suffix));
        let mut reader = BsIoSliceReader::from_slice(&bytes);
        assert_eq!(reader.read_ue().unwrap_err().kind(), ErrorKind::InvalidData);
        assert_eq!(reader.position_in_bits().unwrap(), 129);
    }

    // There is no delimiter; rejection must occur before reading the rest.
    let mut reader = BsIoSliceReader::from_slice(&[0; 16]);
    assert_eq!(reader.read_ue().unwrap_err().kind(), ErrorKind::InvalidData);
    assert_eq!(reader.position_in_bits().unwrap(), 65);
}

#[test]
fn rejects_unsigned_maximum_as_signed() {
    let bytes = bits(&code(
        64,
        "0000000000000000000000000000000000000000000000000000000000000000",
    ));
    let mut reader = BsIoSliceReader::from_slice(&bytes);
    assert_eq!(reader.read_se().unwrap_err().kind(), ErrorKind::InvalidData);
    assert_eq!(reader.position_in_bits().unwrap(), 129);
}

#[test]
fn truncated_prefixes_and_suffixes_return_eof() {
    // Position each incomplete wire at the end of a byte so padding cannot
    // supply a missing delimiter or suffix bit.
    for wire in [
        "",
        "0",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "01",
        "0010",
        "00000000000000000000000000000000000000000000000000000000000000001",
        "00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000",
    ] {
        let padding = (8 - wire.len() % 8) % 8;
        let bytes = bits(&format!("{}{wire}", "1".repeat(padding)));
        let mut reader = BsIoSliceReader::from_slice(&bytes);
        reader.skip_n(padding as u32).unwrap();
        assert_eq!(
            reader.read_ue().unwrap_err().kind(),
            ErrorKind::UnexpectedEof
        );

        let mut reader = BsIoSliceReader::from_slice(&bytes);
        reader.skip_n(padding as u32).unwrap();
        assert_eq!(
            reader.read_se().unwrap_err().kind(),
            ErrorKind::UnexpectedEof
        );
    }
}

#[test]
fn writes_manual_unsigned_vectors() {
    for (value, wire) in ue_vectors() {
        let mut writer = BitstreamIoWriter::with_capacity(0);
        writer.write_ue(value).unwrap();
        // A sentinel also verifies the unpadded length of the encoded value.
        writer.write_bit(true).unwrap();
        writer.byte_align().unwrap();
        assert_eq!(writer.into_inner(), bits(&format!("{wire}1")), "{value}");
    }
}

#[test]
fn writes_manual_signed_vectors() {
    for (value, wire) in se_vectors() {
        let mut writer = BitstreamIoWriter::with_capacity(0);
        writer.write_se(value).unwrap();
        writer.write_bit(true).unwrap();
        writer.byte_align().unwrap();
        assert_eq!(writer.into_inner(), bits(&format!("{wire}1")), "{value}");
    }
}

#[test]
fn rejects_signed_minimum_without_changing_aligned_or_buffered_output() {
    for prefix in ["", "101", "10100101", "101001011"] {
        let mut writer = BitstreamIoWriter::with_capacity(0);
        for bit in prefix.bytes() {
            writer.write_bit(bit == b'1').unwrap();
        }
        let aligned = writer.byte_aligned();
        let before = writer.as_slice().map(<[u8]>::to_vec);
        assert_eq!(
            writer.write_se(i64::MIN).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
        assert_eq!(writer.byte_aligned(), aligned);
        assert_eq!(writer.as_slice().map(<[u8]>::to_vec), before);

        // If rejection touched buffered bits, this final byte comparison fails.
        writer.write_bit(true).unwrap();
        writer.byte_align().unwrap();
        assert_eq!(writer.into_inner(), bits(&format!("{prefix}1")));
    }
}
