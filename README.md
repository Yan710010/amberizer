# Amberizer 琥珀化

> [!NOTE]  
> 本仓库仅作引流，主要开发位于[Codeberg上面的仓库](https://codeberg.org/Yan710010/amberizer)

将聊天记录转换为 markdown 文档，基于 onebot v11 协议的 qq 机器人。

## 嵌套合并转发

支持读取 NapCat 已返回的内层 `forward.data.content`，递归保留内层文字、图片、视频及文档引用。内层 ID 可能不能单独调用 `get_forward_msg`，因此优先使用随外层返回的消息；没有内嵌内容时才回退到 ID 查询。

最多展开 16 层内层转发。超过上限或无法取得内容时，保留“合并转发(无法解析)”提示，并记录原因。此功能不会恢复已过期、服务器不可访问或 NapCat 未提供的媒体。

运行回归测试：`cargo test --locked`。测试使用合成消息，不需要 QQ 登录，也不下载真实媒体。图片和视频的实际可用性仍取决于 QQ 返回的资源。

## 使用例
[由本程序生成的记录](doc/example/例/聊天记录_2026-05-28_23-16.md)  
![私聊使用](doc/example/彳亍.png)
![群聊使用](doc/example/其实是涩图批量下载工具.png)

## 构建

`cargo build --release`

产物位于 target/release/

或从[此处](https://github.com/Yan710010/amberizer/releases/latest)下载自动构建（Windows用户下载 amberizer.exe 即可）

## 运行

可以通过环境变量或[配置文件](amberizer.conf.md)设置 amberizer 的部分选项，包括

| 名 | 例 | 默 | 用 |
| --- | --- | --- | --- |
| AMBERIZER_HOST | [::1] | 127.0.0.1 | websocket 连接地址 |
| AMBERIZER_PORT | 3001 | 17210 | websocket 连接端口 |
| AMBERIZER_TOKEN | 55yL5oeC55qE5Y+Y54yr5aiYCg== | - | websocket token，未设置时可留空 |
| AMBERIZER_CACHE | /tmp/amberizer/cache | ./cache | 存放解析时产生的文件的目录，设置时必须存在 |
| AMBERIZER_CTR_CACHE | /cache | - | NapCat 容器内映射的cache目录，留空时使用与`AMBERIZER_CACHE`相同的目录 |
| AMBERIZER_CMD | 生成琥珀 | 帮帮我吧松树大人 | 在群聊中调用时使用的指令正文 |

> 本程序修改了[onebot_v11](https://docs.rs/onebot_v11/)的部分代码以适配 NapCat 的合并转发消息获取接口，因此可能仅适用于 NapCat  
> ~~除非别的qq onebot框架也用了 NapCat 的这个私有 API 格式~~

## 使用方式

群聊：引用需要琥珀化的合并转发消息，@机器人，正文为「帮帮我吧松树大人」（或自定义的指令）

私聊：直接发送合并转发

如果转发内容仅包含文本信息，bot 会直接发送 聊天记录.md 文件

如果转发内容包含图片、视频等，bot会按顺序下载，并最终打包为 zip 进行发送


## 已知问题

- NapCat 未返回内层内容且内层 ID 不可单独查询时，该层仍无法解析
- 无法正确获取发送者的QQ号，以及有概率丢失用户名（回退为"QQ用户"）（不过似乎已经[修了](https://github.com/NapNeko/NapCatQQ/issues/1827)...?）
- B站视频分享卡片等数据复杂度稍高且格式不统一（NapCat直接以原始json格式发送内容），暂未解析
