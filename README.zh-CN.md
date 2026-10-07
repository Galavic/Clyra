# Clyra

你跑的每个编码 agent（Claude Code、Codex、Cursor、Devin、Grok、Hermes、Pi、Antigravity），一个窗口全搞定——默认跑在你自己的机器上，也可以在设备之间同步。

*[English](README.md) | 简体中文*

![Clyra 驱动一个 Claude Code 会话，侧边栏是实时的分支 diff](apps/landing/public/assets/app-screenshot.jpg)

每台设备各跑一个小引擎，会话就存在这台设备上。装完默认是纯本地模式，不用账号，也不用联网。

## 桌面安装包（Windows、macOS、Linux）

在 [Clyra installers](https://github.com/Galavic/Clyra/actions/workflows/installers.yml) 构建并下载桌面安装包。
Windows 用 setup `.exe`，macOS 按芯片用 `.dmg`（Apple silicon 或 Intel），Linux 按架构用
`.deb` 或便携版 `.tar.gz`（x64 或 ARM64）。做法见[安装说明](dist/INSTALLERS.md)。

## 在本地安装运行（Linux）

```bash
# 先装下载好的桌面包：
sudo apt install ./clyra-0.2.83-linux-x86_64.deb
clyra status
```

直接从应用菜单打开 Clyra 就行，本地用完全不用登录。要装常驻的用户守护进程，跑 `clyra daemon install`。

日常命令：

```bash
clyra status      # 查看本地/同步模式和引擎状态
clyra update      # 更新到最新版本
clyra daemon start|stop|restart|status
```

## 可选：多设备同步

只有想打开账号下的同步工作区时才需要登录。登录会换掉引擎下次启动时用的 profile，所以改之前先停掉守护进程：

```bash
clyra daemon stop
clyra login
clyra daemon start
```

之后就可以在一台同步过的设备上起 agent，换另一台设备接着看、接着操作。一台常开的机器，比如 VPS，可以在你合上笔记本之后继续跑这些 agent。

登录不会上传、搬走或导入已有的本地会话。本地会话和它们的附件仍然留在本地 profile 下，切回纯本地模式时会照常出现：

```bash
clyra daemon stop
clyra logout
clyra daemon start
```

如果有引擎正占着数据目录，`clyra login` 和 `clyra logout` 会拒绝改动凭据。桌面应用同样遵守这条边界：profile 要等下次重启才切换。

macOS 上用桌面版发行包，或者从源码构建 `clyra`，再运行 `clyra daemon install` 装上 launchd 服务。

Windows 上打开 setup 安装包就行，另外也有便携版 ZIP，解压后直接跑
`clyra.exe`。从源码构建看[开发说明](docs/reference/windows-development.md)。

---

想参与开发，或者好奇它怎么跑起来的？[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/Galavic/Clyra)，也可以看 [ARCHITECTURE.md](ARCHITECTURE.md)。

采用 [MIT License](LICENSE)。
