use std::future::Future;

use onebot_v11::event::message::MiscMessage;

use crate::error::Error;

pub const MAX_FORWARD_DEPTH: usize = 16;

/// Inner IDs can be local NapCat cache IDs, so reuse embedded content before querying.
pub async fn messages_or_fetch<F>(
    embedded: Option<Vec<MiscMessage>>,
    depth: usize,
    fetch: F,
) -> Result<Vec<MiscMessage>, Error>
where
    F: Future<Output = Result<Vec<MiscMessage>, Error>>,
{
    if depth > MAX_FORWARD_DEPTH {
        return Err(Error::ForwardDepthLimit(MAX_FORWARD_DEPTH));
    }
    match embedded {
        Some(messages) if !messages.is_empty() => Ok(messages),
        _ => fetch.await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use onebot_v11::{MessageSegment, message::segment::ForwardData};
    use serde_json::{Value, json};

    fn message(segments: Value) -> Value {
        json!({
            "time": 1, "self_id": 10000, "post_type": "message",
            "message_type": "private", "sub_type": "friend", "message_id": 1,
            "user_id": 10001, "message": segments, "raw_message": "",
            "sender": {"nickname": "Example", "user_id": 10001}
        })
    }

    fn example_messages() -> Vec<MiscMessage> {
        serde_json::from_value(json!([message(json!([
            {"type": "text", "data": {"text": "example"}}
        ]))]))
        .unwrap()
    }

    #[tokio::test]
    async fn embedded_nested_media_do_not_query_inner_ids() {
        let deepest = message(json!([
            {"type": "image", "data": {"file": "example.jpg", "url": "https://example.invalid/image"}},
            {"type": "video", "data": {"file": "example.mp4", "url": "https://example.invalid/video"}}
        ]));
        let middle = message(json!([
            {"type": "forward", "data": {"id": "inner", "content": [deepest]}}
        ]));
        let forward: ForwardData = serde_json::from_value(json!({
            "id": "outer", "content": [middle]
        }))
        .unwrap();
        let messages = messages_or_fetch(forward.content, 1, async {
            panic!("NapCat rejects this ID; embedded messages must be used")
        })
        .await
        .unwrap();
        let MessageSegment::Forward { data } = &messages[0].message[0] else {
            panic!("nested forward was lost")
        };
        let inner = messages_or_fetch(data.content.clone(), 2, async {
            panic!("the second nested ID must not be fetched either")
        })
        .await
        .unwrap();
        assert!(matches!(inner[0].message[0], MessageSegment::Image { .. }));
        assert!(matches!(inner[0].message[1], MessageSegment::Video { .. }));
    }

    #[tokio::test]
    async fn missing_and_empty_content_fall_back_to_api() {
        for embedded in [None, Some(Vec::new())] {
            let expected = example_messages();
            let actual = messages_or_fetch(embedded, 0, async { Ok(expected.clone()) })
                .await
                .unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[tokio::test]
    async fn fetch_errors_are_preserved() {
        assert!(matches!(
            messages_or_fetch(None, 0, async { Err(Error::NotForward) }).await,
            Err(Error::NotForward)
        ));
    }

    #[tokio::test]
    async fn depth_limit_stops_before_fetching() {
        assert!(matches!(
            messages_or_fetch(None, MAX_FORWARD_DEPTH + 1, async {
                panic!("over-depth forwards must not be fetched")
            })
            .await,
            Err(Error::ForwardDepthLimit(MAX_FORWARD_DEPTH))
        ));
        assert!(
            messages_or_fetch(Some(example_messages()), MAX_FORWARD_DEPTH, async {
                panic!("embedded content at the boundary must still work")
            })
            .await
            .is_ok()
        );
    }

    #[test]
    fn id_only_and_null_content_remain_compatible() {
        for value in [
            json!({"id": "example"}),
            json!({"id": "example", "content": null}),
        ] {
            let forward: ForwardData = serde_json::from_value(value).unwrap();
            assert_eq!(forward.content, None);
        }
        assert_eq!(
            serde_json::to_value(MessageSegment::forward("example")).unwrap(),
            json!({"type": "forward", "data": {"id": "example"}})
        );
    }
}
