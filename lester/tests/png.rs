use lester::ImageSurface;
use std::error::Error;
use std::io;

#[test]
fn round_trip_png() {
    static PNG_BYTES: &[u8] = include_bytes!("pattern_4x4.png");
    let mut surface = ImageSurface::read_from_png(PNG_BYTES).unwrap();

    fn assert_expected_pixels(pixels: lester::Argb32Pixels) {
        assert_eq!(pixels.width, 4);
        assert_eq!(pixels.height, 4);
        // ARGB32
        const RED: u32 = 0xFFFF_0000;
        const BLUE: u32 = 0xFF00_00FF;
        #[rustfmt::skip]
        assert_eq!(
            pixels.buffer,
            &[
                RED,  BLUE, BLUE, BLUE,
                BLUE, BLUE, BLUE, BLUE,
                BLUE, BLUE, BLUE, BLUE,
                BLUE, BLUE, BLUE, BLUE,
            ]
        );
    }

    assert_expected_pixels(surface.pixels());

    let mut bytes = Vec::new();
    surface.write_to_png(&mut bytes).unwrap();

    let mut surface2 = ImageSurface::read_from_png(&*bytes).unwrap();
    assert_expected_pixels(surface2.pixels());
}

#[test]
fn zero_bytes_png() {
    expect_io_error_kind(
        ImageSurface::read_from_png("".as_bytes()),
        io::ErrorKind::UnexpectedEof,
    )
}

#[test]
fn invalid_png() {
    let bytes: &[u8] = b"\x89PNG\rnot";
    match ImageSurface::read_from_png(bytes) {
        Err(lester::LesterError::Cairo(ref err)) if err.description() == "out of memory" => {}
        Err(err) => panic!("expected 'out of memory' error, got {:?}", err),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn forward_read_error() {
    struct InvalidDataRead;

    impl io::Read for InvalidDataRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::InvalidData.into())
        }
    }

    expect_io_error_kind(
        ImageSurface::read_from_png(InvalidDataRead),
        io::ErrorKind::InvalidData,
    )
}

#[test]
fn forward_write_error() {
    struct InvalidDataWrite;

    impl io::Write for InvalidDataWrite {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::InvalidData.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let surface = ImageSurface::new_rgb24(4, 4).unwrap();
    expect_io_error_kind(
        surface.write_to_png(InvalidDataWrite),
        io::ErrorKind::InvalidData,
    )
}

fn expect_io_error_kind<T>(result: Result<T, lester::LesterError>, expected_kind: io::ErrorKind) {
    match result {
        Err(lester::LesterError::Io(err)) => assert_eq!(
            err.kind(),
            expected_kind,
            "Expected {:?} error, got {:?}",
            expected_kind,
            err
        ),
        Err(err) => panic!("Expected an IO error, got {:?}", err),
        Ok(_) => panic!("Expected an error"),
    }
}

#[test]
#[should_panic(expected = "panicking during read callback")]
fn forward_read_panic() {
    struct PanickingRead;

    impl io::Read for PanickingRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("panicking during read callback")
        }
    }

    unreachable!(ImageSurface::read_from_png(PanickingRead).is_ok())
}

#[test]
#[should_panic(expected = "panicking during write callback")]
fn forward_write_panic() {
    struct PanickingWrite;

    impl io::Write for PanickingWrite {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            panic!("panicking during write callback")
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let surface = ImageSurface::new_rgb24(4, 4).unwrap();
    unreachable!(surface.write_to_png(PanickingWrite).is_ok())
}
