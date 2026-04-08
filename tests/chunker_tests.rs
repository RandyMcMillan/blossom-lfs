use blossom_lfs::chunking::{ChunkAssembler, Chunker};
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

fn run_async<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::runtime::Runtime::new().unwrap().block_on(fut)
}

#[test]
fn test_chunk_small_file() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"test content for chunking").unwrap();
    file.flush().unwrap();

    let chunker = Chunker::new(10).unwrap();
    let (chunks, size) = run_async(async { chunker.chunk_file(file.path()).await.unwrap() });

    assert_eq!(size, 25, "File should be 25 bytes");
    assert!(chunks.len() >= 2, "Should split into at least 2 chunks");
    assert!(chunks[0].size <= 10, "Each chunk should be <= chunk_size");
}

#[test]
fn test_chunk_large_file() {
    let mut file = NamedTempFile::new().unwrap();
    let data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    file.write_all(&data).unwrap();
    file.flush().unwrap();

    let chunker = Chunker::new(16).unwrap();
    let (chunks, size) = run_async(async { chunker.chunk_file(file.path()).await.unwrap() });

    assert_eq!(size, 1024);
    assert!(
        chunks.len() >= 64,
        "Should have at least 64 chunks for 1KB with 16 byte chunks"
    );
}

#[test]
fn test_chunk_hashing() {
    let chunker = Chunker::new(16).unwrap();
    let hash1 = chunker.hash_chunk(b"hello world");
    let hash2 = chunker.hash_chunk(b"hello world");
    let hash3 = chunker.hash_chunk(b"goodbye world");

    assert_eq!(hash1, hash2, "Same data should produce same hash");
    assert_ne!(hash1, hash3, "Different data should produce different hash");
    assert_eq!(hash1.len(), 64, "SHA256 hash should be 64 hex chars");
}

#[test]
fn test_should_chunk() {
    let chunker = Chunker::new(1024).unwrap();

    assert!(!chunker.should_chunk(512), "Small file should not chunk");
    assert!(
        !chunker.should_chunk(1024),
        "Exactly chunk_size should not chunk"
    );
    assert!(chunker.should_chunk(2048), "Large file should chunk");
}

#[test]
fn test_read_chunk() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"0123456789ABCDEFGHIJ").unwrap();
    file.flush().unwrap();

    let chunker = Chunker::new(10).unwrap();

    let chunk =
        tokio_test::block_on(async { chunker.read_chunk(file.path(), 10, 10).await.unwrap() });

    assert_eq!(&chunk, b"ABCDEFGHIJ", "Should read correctchunk");
}

#[test]
fn test_chunk_offsets() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"0123456789ABCDEFGHIJ").unwrap();
    file.flush().unwrap();

    let chunker = Chunker::new(10).unwrap();
    let (chunks, size) =
        tokio_test::block_on(async { chunker.chunk_file(file.path()).await.unwrap() });

    assert_eq!(size, 20);
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].offset, 0);
    assert_eq!(chunks[0].size, 10);
    assert_eq!(chunks[1].offset, 10);
    assert_eq!(chunks[1].size, 10);
}

#[test]
fn test_chunker_zero_size_error() {
    let result = Chunker::new(0);
    assert!(result.is_err());
}

#[test]
fn test_chunker_chunk_size_accessor() {
    let chunker = Chunker::new(42).unwrap();
    assert_eq!(chunker.chunk_size(), 42);
}

#[test]
fn test_chunk_exact_boundary() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"0123456789").unwrap(); // exactly 10 bytes
    file.flush().unwrap();

    let chunker = Chunker::new(10).unwrap();
    let (chunks, size) =
        tokio_test::block_on(async { chunker.chunk_file(file.path()).await.unwrap() });

    assert_eq!(size, 10);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].size, 10);
    assert_eq!(chunks[0].offset, 0);
}

#[test]
fn test_chunk_empty_file() {
    let file = NamedTempFile::new().unwrap();

    let chunker = Chunker::new(10).unwrap();
    let (chunks, size) =
        tokio_test::block_on(async { chunker.chunk_file(file.path()).await.unwrap() });

    assert_eq!(size, 0);
    assert!(chunks.is_empty());
}

#[test]
fn test_assembler_write_and_assemble() {
    let tmp = TempDir::new().unwrap();
    let assembler = ChunkAssembler::new(tmp.path().to_path_buf());

    run_async(async {
        assembler.write_chunk("testfile", 0, b"AAAA").await.unwrap();
        assembler.write_chunk("testfile", 1, b"BBBB").await.unwrap();
        assembler.write_chunk("testfile", 2, b"CCCC").await.unwrap();
    });

    let output = tmp.path().join("assembled.bin");
    run_async(async {
        assembler.assemble("testfile", &output, 3).await.unwrap();
    });

    let data = std::fs::read(&output).unwrap();
    assert_eq!(&data, b"AAAABBBBCCCC");

    run_async(async {
        assembler.cleanup("testfile").await.unwrap();
    });

    assert!(!tmp.path().join("testfile").exists());
}

#[test]
fn test_assembler_multiple_files() {
    let tmp = TempDir::new().unwrap();
    let assembler = ChunkAssembler::new(tmp.path().to_path_buf());

    run_async(async {
        assembler.write_chunk("file_a", 0, b"AAA").await.unwrap();
        assembler.write_chunk("file_b", 0, b"BBB").await.unwrap();
    });

    let out_a = tmp.path().join("a.bin");
    let out_b = tmp.path().join("b.bin");

    run_async(async {
        assembler.assemble("file_a", &out_a, 1).await.unwrap();
        assembler.assemble("file_b", &out_b, 1).await.unwrap();
    });

    assert_eq!(std::fs::read(&out_a).unwrap(), b"AAA");
    assert_eq!(std::fs::read(&out_b).unwrap(), b"BBB");
}
