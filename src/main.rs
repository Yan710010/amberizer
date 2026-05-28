use std::{
    collections::HashMap,
    env::var,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_recursion::async_recursion;
use chrono::{Datelike, FixedOffset, Timelike};
use onebot_v11::{
    Event, MessageSegment as MS,
    api::{
        payload::{ApiPayload, GetForwardMsg, GetMsg, SendGroupMsg, SendPrivateMsg},
        resp::ApiRespData,
    },
    connect::{
        WsType,
        ws::{WsConfig, WsConnect},
    },
    event::message::Message,
};
use reqwest::Client;
use tokio::io::AsyncReadExt;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{error::Error, util::index_resource};

mod error;
mod util;

enum Target {
    Group(i64),
    Private(i64),
}

#[tokio::main]
async fn main() {
    // 处理用户设置
    let host = var("AMBERIZER_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = var("AMBERIZER_PORT").map_or(17210, |s| {
        s.parse().expect("AMBERIZER_PORT需要设置为0~65535的数字")
    });
    let access_token = var("AMBERIZER_TOKEN").ok();
    let cache_dir = var("AMBERIZER_CACHE")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("cache"));
    let container_base_cache_dir = var("AMBERIZER_CTR_CACHE").ok();
    let command = var("AMBERIZER_CMD").unwrap_or_else(|_| "帮帮我吧松树大人".into());

    let connect = WsConnect::new(WsConfig {
        host,
        port,
        r#type: WsType::Universal,
        bot_id: None,
        bot_nick_name: None,
        access_token,
    })
    .await
    .unwrap();
    println!("成功连接到NapCat");
    let mut rx = connect.subscribe().await;

    // 初始化 http client
    let client = Client::new();

    while let Ok(event) = rx.recv().await {
        match event {
            Event::Message(message) => match message {
                Message::PrivateMessage(message) => {
                    if let Some(MS::Forward { data }) = message.message.first() {
                        if let Err(e) = process(
                            connect.clone(),
                            &client,
                            &data.id,
                            Target::Private(message.user_id),
                            &cache_dir,
                            container_base_cache_dir.clone(),
                        )
                        .await
                        {
                            let _ = connect
                                .clone()
                                .call_api(ApiPayload::SendPrivateMsg(SendPrivateMsg {
                                    user_id: message.user_id,
                                    message: Vec::from([MS::text(format!("解析失败: {e}"))]),
                                    auto_escape: false,
                                }))
                                .await;
                        }
                    }
                }
                Message::GroupMessage(message) => {
                    // 检查是否符合指令调用
                    if let Some(MS::Reply { data }) = message.message.first()
                        && let Ok(id) = data.id.parse()
                        && let Some(MS::At { data }) = message.message.get(1)
                        && data.qq == message.self_id.to_string()
                        && let Some(MS::Text { data }) = message.message.get(2)
                        && data.text.trim() == command
                    {
                        // 获取引用的消息
                        let api_resp = connect
                            .clone()
                            .call_api(ApiPayload::GetMsg(GetMsg { message_id: id }))
                            .await;
                        //dbg!(&api_resp);
                        if let Ok(resp) = api_resp
                            && let ApiRespData::GetMsgResponse(res) = resp.data
                            && let Some(MS::Forward { data }) = res.message.first()
                        {
                            if let Err(e) = process(
                                connect.clone(),
                                &client,
                                &data.id,
                                Target::Group(message.group_id),
                                &cache_dir,
                                container_base_cache_dir.clone(),
                            )
                            .await
                            {
                                let _ = connect
                                    .clone()
                                    .call_api(ApiPayload::SendGroupMsg(SendGroupMsg {
                                        group_id: message.group_id,
                                        message: vec![MS::text(format!("解析失败: {e}"))],
                                        auto_escape: false,
                                    }))
                                    .await;
                            };
                        }
                    }
                }
            },
            Event::ApiRespBuilder(api_resp_builder) => {
                dbg!(api_resp_builder);
            }
            _ => {}
        }
    }
}

async fn process(
    connect: Arc<WsConnect>,
    client: &Client,
    msg_id: &str,
    target: Target,
    cache: &Path,
    ctr_cache: Option<String>,
) -> Result<(), Error> {
    // 首先清理上回发送的缓存
    for file in std::fs::read_dir("cache")?.flatten() {
        if let Err(file) = tokio::fs::remove_file(file.path()).await {
            eprintln!("删除缓存时失败: {file}");
        }
    }

    let mut index = 1u64;
    let doc = process_nested(connect.clone(), client, msg_id, &mut index, cache).await?;

    let timezone = FixedOffset::east_opt(8 * 3600).unwrap();
    let filename = format!(
        "聊天记录_{}.md",
        chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::now())
            .with_timezone(&timezone)
            .format("%Y-%m-%d_%H-%M")
    );
    tokio::fs::write(cache.join(&filename), doc).await?;

    //  检查多文件并打包
    let files = std::fs::read_dir(cache)?
        .filter_map(|f| f.ok())
        .map(|f| f.path())
        .collect::<Vec<_>>();
    let filename = if files.len() > 1 {
        // 有额外的资源需要打包
        let archive_filename = format!(
            "聊天记录归档_{}.zip",
            chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::now())
            .with_timezone(&timezone)
                .format("%Y-%m-%d_%H-%M")
        );
        let mut zip_file = std::fs::File::create(cache.join(&archive_filename))
            .map_err(|e| Error::ArchiveFailed(e.into()))?;
        let mut zip = ZipWriter::new(&mut zip_file);
        for path in files {
            if let Some(filename) = path.file_name()
                && let Ok(mut file) = tokio::fs::File::open(&path).await
            {
                let time = file
                    .metadata()
                    .await
                    .and_then(|m| m.modified())
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).with_timezone(&timezone))
                    .unwrap_or_default();
                let opt = SimpleFileOptions::default().last_modified_time(
                    zip::DateTime::from_date_and_time(
                        time.year() as u16,
                        time.month() as u8,
                        time.day() as u8,
                        time.hour() as u8,
                        time.minute() as u8,
                        time.second() as u8,
                    )
                    .unwrap_or_default(),
                );
                zip.start_file(filename.to_string_lossy(), opt)
                    .map_err(|e| Error::ArchiveFailed(e.into()))?;
                let mut buf = vec![0u8; 2 * 1024 * 1024];
                while let Ok(size) = file.read(&mut buf).await
                    && size > 0
                {
                    zip.write_all(&buf[..size])
                        .map_err(|e| Error::ArchiveFailed(e.into()))?;
                }
            }
        }
        zip.finish().map_err(|e| Error::ArchiveFailed(e.into()))?;
        archive_filename
    } else {
        filename
    };

    let send_path = format!(
        "{}/{filename}",
        ctr_cache.unwrap_or(cache.to_string_lossy().to_string())
    );
    eprintln!("发送文件 {send_path}");

    let payload = match target {
        Target::Group(id) => ApiPayload::SendGroupMsg(SendGroupMsg {
            group_id: id,
            auto_escape: false,
            message: Vec::from([MS::file(send_path, Option::<&str>::None)]),
        }),
        Target::Private(id) => ApiPayload::SendPrivateMsg(SendPrivateMsg {
            user_id: id,
            auto_escape: false,
            message: Vec::from([MS::file(send_path, Option::<&str>::None)]),
        }),
    };
    connect.call_api(payload).await?;
    Ok(())
}

#[async_recursion]
async fn process_nested(
    connect: Arc<WsConnect>,
    client: &Client,
    msg_id: &str,
    index: &mut u64,
    cache: &Path,
) -> Result<String, Error> {
    eprintln!("获取合并转发{msg_id}内容...");
    // 获取合并转发内容
    let ApiRespData::GetForwardMsgResponse(data) = connect
        .clone()
        .call_api(ApiPayload::GetForwardMsg(GetForwardMsg {
            id: msg_id.into(),
        }))
        .await?
        .data
    else {
        return Err(Error::NotForward);
    };
    // 获取 qid name 对照表
    let qq_name_map: HashMap<i64, String> = data
        .messages
        .iter()
        .map(|m| (m.sender.user_id, m.sender.nickname.clone()))
        .collect();

    // 绝赞遍历消息列表
    let mut doc = String::new();
    for msg in data.messages {
        let msg_time = chrono::DateTime::from_timestamp_secs(msg.time)
            .unwrap_or_default()
            .with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());

        //dbg!(&msg);
        // 该用户发送的消息
        doc += &format!(
            "{} <!-- {} --> {}:\n",
            msg.sender.nickname,
            // 处理 napcat 设置的占位符
            if msg.sender.user_id != 1094950020 {
                msg.sender.user_id
            } else {
                0
            },
            msg_time.format("%Y-%m-%d %H:%M:%S")
        );
        // 遍历消息内容
        for ms in msg.message {
            match ms {
                MS::Text { data } => doc += &data.text,
                MS::Face { data } => doc += util::face_to_str(&data.id),
                MS::MFace { data } => {
                    // 下载图片到本地
                    match util::download_resource(
                        client,
                        data.url,
                        index_resource(index, &data.summary),
                        cache,
                        msg.time as u64,
                    )
                    .await
                    {
                        Ok(file) => {
                            doc += &format!("\n![{}]({})\n", data.summary, file);
                        }
                        Err(e) => {
                            eprintln!("解析图片时出错: {e}");
                            doc += &format!("\n{}\n", data.summary);
                        }
                    }
                }
                MS::At { data } => {
                    doc += if data.qq == "all" {
                        "全体成员"
                    } else if let Ok(id) = data.qq.parse()
                        && let Some(name) = qq_name_map.get(&id)
                    {
                        name
                    } else {
                        &data.qq
                    }
                }
                MS::Image { data } => {
                    // 下载图片到本地
                    doc += &if let Some(url) = data.url {
                        let summary = data
                            .summary
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| "[图片]".into());
                        match util::download_resource(
                            client,
                            url,
                            index_resource(index, &data.file),
                            cache,
                            msg.time as u64,
                        )
                        .await
                        {
                            Ok(file) => {
                                format!("![{}]({})\n", summary, file)
                            }
                            Err(e) => {
                                eprintln!("解析图片时出错: {e}");
                                format!("\n{}\n", summary)
                            }
                        }
                    } else {
                        "\n[图片]\n".into()
                    }
                }
                MS::Record { data } => {
                    doc += &if let Some(url) = data.url
                        && let Ok(file) = util::download_resource(
                            client,
                            url,
                            index_resource(index, &data.file),
                            cache,
                            msg.time as u64,
                        )
                        .await
                    {
                        format!("[[语音]]({file})\n")
                    } else {
                        "[语音]\n".into()
                    };
                }
                MS::Video { data } => {
                    doc += &if let Some(url) = data.url {
                        match util::download_resource(
                            client,
                            url,
                            index_resource(index, &data.file),
                            cache,
                            msg.time as u64,
                        )
                        .await
                        {
                            Ok(file) => {
                                format!("[[视频]]({})\n", file)
                            }
                            Err(e) => {
                                eprintln!("解析视频时出错: {e}");
                                "[视频]\n".into()
                            }
                        }
                    } else {
                        "[视频]\n".into()
                    };
                }
                MS::File { data } => {
                    doc += &if let Some(url) = data.url {
                        match util::download_resource(
                            client,
                            url,
                            index_resource(index, &data.file),
                            cache,
                            msg.time as u64,
                        )
                        .await
                        {
                            Ok(file) => format!("[[文件]]({file})\n"),
                            Err(e) => {
                                eprintln!("下载文件时出错: {e}");
                                "[文件]\n".into()
                            }
                        }
                    } else {
                        "[文件]\n".into()
                    }
                }
                MS::Share { data } => {
                    doc += &format!("[分享: {}]({})", data.title, data.url);
                    if let Some(c) = data.content {
                        doc += &format!("  {c}");
                    }
                    if let Some(i) = data.image {
                        doc += &format!(" [!图标]({i})")
                    }
                    doc += "\n";
                }
                MS::Contact { data } => {
                    doc += &match data.r#type {
                        onebot_v11::message::segment::ContactType::Group => {
                            format!("推荐群聊: {}\n", data.id)
                        }
                        onebot_v11::message::segment::ContactType::QQ => {
                            format!("推荐用户: {}\n", data.id)
                        }
                    };
                }
                MS::Location { data } => {
                    doc += &format!("分享位置: ({} , {})\n", data.lat, data.lon);
                    if let Some(title) = data.title {
                        doc += &title;
                        doc += "  ";
                    }
                    if let Some(content) = data.content {
                        doc += &content;
                    }
                    doc += "\n";
                }
                MS::Music { data } => {
                    doc += &format!("分享{}音乐: id{}\n", data.r#type, data.id);
                }
                MS::CustomMusic { data } => {
                    // 写了一部分之后才发现 onebot 文档说这是仅发送数据
                    doc += &format!(
                        "分享音乐: [{}]({}) [音频]({})",
                        data.title, data.url, data.audio
                    );
                    if let Some(content) = data.content {
                        doc += &format!("\n{content}");
                    }
                    if let Some(image) = data.image {
                        doc += &format!("\n[!图标]({image})");
                    }
                    doc.push('\n');
                }
                MS::Reply { data } => {
                    doc += &format!("引用 <!-- id: {} 暂无法解析真实引用内容 -->\n", data.id);
                }
                MS::Forward { data } => {
                    match process_nested(connect.clone(), client, &data.id, index, cache).await {
                        Ok(nested_doc) => {
                            // 写入单独的文件，然后正文里提供引用
                            let file = format!("{index}_Forward.md");
                            match tokio::fs::write(cache.join(&file), nested_doc).await {
                                Ok(_) => {
                                    *index += 1;
                                    doc += &format!("[合并转发]({file})");
                                }
                                Err(e) => {
                                    eprintln!("写入合并转发文件时出错: {e}");
                                    doc += "[合并转发(无法解析)]";
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("处理嵌套合并转发时遇到错误: {e}");
                            doc += "[合并转发(无法解析)]";
                        }
                    }
                }
                _ => {}
            }
        }
        doc += "\n\n";
    }
    Ok(doc)
}
