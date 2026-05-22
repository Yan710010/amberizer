# Amberizer 琥珀化

> [!NOTE]  
> 此github仓库为[源仓库](https://codeberg.org/Yan710010/amberizer)镜像

将聊天记录转换为 markdown 文档，基于 onebot v11 协议的 qq 机器人。

# 构建

`cargo build --release`

产物位于 target/release/

# 兼容性

It works on my machine.

现阶段该程序完全仅服务于我的设备环境:
- 使用`podman`或`docker`运行的`NapCat`
- 在程序运行文件夹下存在 cache/ 目录，且容器中该目录被绑定至 /cache/
- 在 127.0.0.1:17210 监听的 NapCat 正向 WebSocket 连接，token 为 `YesURRightButGenshinImpactIs__`
- Archlinux（由于我没有在 Windows 上使用过 docker 等容器技术，不确定运作方式是否相似）

如果想要自己运行的话请务必注意以上限制，或者自行修改 src/main.rs 中对于这些路径的定义。

> 本程序修改了[onebot_v11](https://docs.rs/onebot_v11/)的部分代码以适配 NapCat 的合并转发消息获取接口，因此可能仅适用于 NapCat  
> ~~除非别的qq onebot框架也用了 NapCat 的这个私有 API 格式~~

# 使用方式

群聊：引用需要琥珀化的合并转发消息，@机器人，正文为「帮帮我吧松树大人」

私聊：直接发送合并转发

如果转发内容仅包含文本信息，bot 会直接发送 聊天记录.md 文件

如果转发内容包含图片、视频等，bot会按顺序下载，并最终打包为 zip 进行发送

# 效果图
![私聊使用](doc/example/彳亍.png)
![群聊使用](doc/example/其实是涩图批量下载工具.png)
