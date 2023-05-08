use oneio::{s3_download, s3_list, s3_reader, s3_stats, s3_upload};
use std::io::Read;

/// This example shows how to upload a file to S3 and read it back.
///
/// You need to set the following environment variables (e.g. in .env):
/// - AWS_ACCESS_KEY_ID
/// - AWS_SECRET_ACCESS_KEY
/// - AWS_REGION (e.g. "us-east-1") (use "auto" for Cloudflare R2)
/// - AWS_ENDPOINT
fn main() {
    // upload to S3
    s3_upload("oneio-test", "test/README.md", "README.md").unwrap();

    // read directly from S3
    let mut content = String::new();
    s3_reader("oneio-test", "test/README.md")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    println!("{}", content);

    // download from S3
    s3_download("oneio-test", "test/README.md", "test/README-2.md").unwrap();

    // get S3 file stats
    let res = s3_stats("oneio-test", "test/README.md").unwrap();
    dbg!(res);

    // error if file does not exist
    let res = s3_stats("oneio-test", "test/README___NON_EXISTS.md");
    assert!(res.is_err());

    // list S3 files
    let res = s3_list("oneio-test", "test/", Some("/")).unwrap();
    dbg!(res);
}
