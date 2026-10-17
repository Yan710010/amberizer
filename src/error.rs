use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("网络请求时出错: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("引用的消息不是合并转发")]
    NotForward,
    #[error("合并转发嵌套超过 {0} 层，已停止展开")]
    ForwardDepthLimit(usize),
    #[error("IO出错: {0}")]
    Io(#[from] std::io::Error),
    #[error("归档消息时出现错误: {0}")]
    ArchiveFailed(anyhow::Error),
    // 来来你告诉我你一个库返回 anyhow::Error 是什么意思来
    #[error("（可能是）onebot协议库错误: {0}")]
    Anyhow(#[from] anyhow::Error),
}
