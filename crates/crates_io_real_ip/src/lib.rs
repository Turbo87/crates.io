use http::{HeaderMap, HeaderValue};
use std::iter::Iterator;
use std::net::IpAddr;
use std::str::from_utf8;

mod cloudfront;
mod fastly;

pub use cloudfront::CLOUDFRONT_NETWORKS;
pub use fastly::FASTLY_NETWORKS;

const X_FORWARDED_FOR: &str = "X-Forwarded-For";

fn is_cdn_ip(ip: &IpAddr) -> bool {
    CLOUDFRONT_NETWORKS
        .iter()
        .chain(FASTLY_NETWORKS.iter())
        .any(|trusted_proxy| trusted_proxy.contains(*ip))
}

/// Extracts the client IP address from the `X-Forwarded-For` header.
///
/// This function will return the last valid non-CDN IP address in the
/// `X-Forwarded-For` header, if any. IP addresses belonging to known
/// CDN networks (CloudFront, Fastly) are filtered out.
pub fn process_xff_headers(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get_all(X_FORWARDED_FOR)
        .iter()
        .flat_map(|header| {
            parse_xff_header(header)
                .into_iter()
                .filter_map(|r| r.ok())
                .filter(|ip| !is_cdn_ip(ip))
        })
        .next_back()
}

/// Parses the content of an `X-Forwarded-For` header into a
/// `Vec<Result<IpAddr, &[u8]>>`.
fn parse_xff_header(header: &HeaderValue) -> Vec<Result<IpAddr, &[u8]>> {
    let bytes = header.as_bytes();
    if bytes.is_empty() {
        return vec![];
    }

    bytes
        .split(|&byte| byte == b',')
        .map(|bytes| parse_ip_addr(bytes))
        .collect()
}

fn parse_ip_addr(bytes: &[u8]) -> Result<IpAddr, &[u8]> {
    from_utf8(bytes)
        .map_err(|_| bytes)?
        .trim()
        .parse()
        .map_err(|_| bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[test]
    fn test_process_xff_headers() {
        #[track_caller]
        fn test(input: Vec<&[u8]>, expectation: Option<&str>) {
            let mut headers = HeaderMap::new();
            for value in input {
                let value = HeaderValue::from_bytes(value).unwrap();
                headers.append(X_FORWARDED_FOR, value);
            }

            let expectation: Option<IpAddr> = expectation.map(|ip| ip.parse().unwrap());

            assert_eq!(process_xff_headers(&headers), expectation)
        }

        // Generic behavior
        test(vec![], None);
        test(vec![b""], None);
        test(vec![b"1.1.1.1"], Some("1.1.1.1"));
        test(vec![b"1.1.1.1, 2.2.2.2"], Some("2.2.2.2"));
        test(vec![b"1.1.1.1, 2.2.2.2, 3.3.3.3"], Some("3.3.3.3"));
        test(
            vec![b"oh, hi,,127.0.0.1,,,,, 12.34.56.78  "],
            Some("12.34.56.78"),
        );

        // CloudFront behavior
        test(vec![b"130.176.118.147"], None);
        test(vec![b"1.1.1.1, 130.176.118.147"], Some("1.1.1.1"));
        test(vec![b"1.1.1.1, 2.2.2.2, 130.176.118.147"], Some("2.2.2.2"));

        // Fastly behavior (151.101.0.0/16)
        test(vec![b"151.101.0.1"], None);
        test(vec![b"1.1.1.1, 151.101.0.1"], Some("1.1.1.1"));
        test(vec![b"1.1.1.1, 2.2.2.2, 151.101.0.1"], Some("2.2.2.2"));

        // Multiple headers behavior
        test(vec![b"1.1.1.1, 2.2.2.2", b"3.3.3.3"], Some("3.3.3.3"));
        test(
            vec![b"1.1.1.1, 130.176.118.147", b"3.3.3.3"],
            Some("3.3.3.3"),
        );
    }

    #[test]
    fn test_parse_xff_header() {
        #[track_caller]
        fn test(input: &'static [u8], expectation: Vec<Result<&str, &[u8]>>) {
            let header = HeaderValue::from_bytes(input).unwrap();

            let expectation: Vec<Result<IpAddr, &[u8]>> = expectation
                .into_iter()
                .map(|ip| ip.map(|ip| ip.parse().unwrap()))
                .collect();

            assert_eq!(parse_xff_header(&header), expectation)
        }

        test(b"", vec![]);
        test(b"1.2.3.4", vec![Ok("1.2.3.4")]);
        test(
            b"1.2.3.4, 11.22.33.44",
            vec![Ok("1.2.3.4"), Ok("11.22.33.44")],
        );
        test(
            b"oh, hi,,127.0.0.1,,,,, 12.34.56.78  ",
            vec![
                Err(b"oh"),
                Err(b" hi"),
                Err(b""),
                Ok("127.0.0.1"),
                Err(b""),
                Err(b""),
                Err(b""),
                Err(b""),
                Ok("12.34.56.78"),
            ],
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::{X_FORWARDED_FOR, is_cdn_ip, parse_xff_header, process_xff_headers};
    use hegel::TestCase;
    use hegel::generators as gs;
    use http::{HeaderMap, HeaderValue};
    use std::net::IpAddr;

    /// Independent oracle for `process_xff_headers`: the last successfully parsed,
    /// non-CDN IP across all headers in order, computed directly from std parsing.
    fn expected(headers: &HeaderMap) -> Option<IpAddr> {
        let mut last = None;
        for value in headers.get_all(X_FORWARDED_FOR) {
            for segment in value.as_bytes().split(|&b| b == b',') {
                if let Ok(text) = std::str::from_utf8(segment) {
                    if let Ok(ip) = text.trim().parse::<IpAddr>() {
                        if !is_cdn_ip(&ip) {
                            last = Some(ip);
                        }
                    }
                }
            }
        }
        last
    }

    /// Header value content built from a pool of real IPs (CDN and non-CDN),
    /// junk, and assorted separators (including empties via `,,`). All chars are
    /// printable so `HeaderValue::from_bytes` always succeeds.
    fn xff_value() -> impl hegel::Generator<String> {
        hegel::compose!(|tc| {
            let pool: Vec<String> = [
                "1.1.1.1",
                "2.2.2.2",
                "3.3.3.3",
                "12.34.56.78",
                "130.176.118.147", // CloudFront
                "151.101.0.1",     // Fastly
                "::1",
                "2001:db8::1",
                " 9.9.9.9 ",
                "oh",
                "hi",
                "",
                "127.0.0.1",
                "255.255.255.256", // invalid
            ]
            .into_iter()
            .map(String::from)
            .collect();

            let segments = tc.draw(gs::vecs(gs::sampled_from(pool)).max_size(6));
            let separator = tc.draw(gs::sampled_from(vec![",", ", ", " , ", ",,"]));
            segments.join(separator)
        })
    }

    fn header_map() -> impl hegel::Generator<HeaderMap> {
        hegel::compose!(|tc| {
            let values = tc.draw(gs::vecs(xff_value()).max_size(3));
            let mut headers = HeaderMap::new();
            for value in &values {
                if let Ok(header) = HeaderValue::from_bytes(value.as_bytes()) {
                    headers.append(X_FORWARDED_FOR, header);
                }
            }
            headers
        })
    }

    #[hegel::test(test_cases = 3000)]
    fn prop_process_xff_matches_oracle(tc: TestCase) {
        let headers = tc.draw(header_map());
        let result = process_xff_headers(&headers);
        assert_eq!(result, expected(&headers));
        // The chosen IP is never one of our CDN networks.
        if let Some(ip) = result {
            assert!(!is_cdn_ip(&ip), "returned a CDN IP: {ip}");
        }
    }

    #[hegel::test(test_cases = 3000)]
    fn prop_parse_xff_header_structure(tc: TestCase) {
        let content = tc.draw(xff_value());
        let header = HeaderValue::from_bytes(content.as_bytes()).unwrap();
        let results = parse_xff_header(&header);

        let bytes = header.as_bytes();
        if bytes.is_empty() {
            assert!(results.is_empty());
            return;
        }

        // One result per comma-separated segment, in order.
        let segments: Vec<&[u8]> = bytes.split(|&b| b == b',').collect();
        assert_eq!(results.len(), segments.len());

        for (result, segment) in results.iter().zip(segments) {
            let parsed = std::str::from_utf8(segment).unwrap().trim().parse::<IpAddr>();
            match result {
                Ok(ip) => assert_eq!(parsed.unwrap(), *ip),
                Err(raw) => {
                    assert_eq!(*raw, segment, "Err must preserve the original bytes");
                    assert!(parsed.is_err(), "segment {segment:?} parsed but was Err");
                }
            }
        }
    }
}
