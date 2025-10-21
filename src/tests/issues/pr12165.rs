use crate::util::{RequestHelper, TestApp};
use bytes::Bytes;
use crates_io_test_utils::builders::PublishBuilder;
use insta::{assert_json_snapshot, assert_snapshot};

/// See <https://github.com/rust-lang/crates.io/pull/12165>.
#[tokio::test(flavor = "multi_thread")]
async fn test_issue_2736() {
    let (_app, _, user) = TestApp::full()
        .with_config(|c| c.max_upload_size = u32::MAX)
        .with_config(|c| c.max_unpack_size = u64::MAX)
        .with_user()
        .await;

    let bytes = include_bytes!("malicious-crate-0.1.0.crate");
    let bytes = Bytes::from_static(bytes);

    let (json, _tarball) = PublishBuilder::new("malicious-crate", "0.1.0").build();
    let body = PublishBuilder::create_publish_body(&json, &bytes);

    let response = user.publish_crate(body).await;
    assert_snapshot!(response.status(), @"400 Bad Request");
    assert_json_snapshot!(response.text(), @"");
}
