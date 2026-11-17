# Serper

[English](README.md) | 简体中文

Serper 是一个使用 Rust 编写的 Serper API 命令行客户端。它支持搜索网页、图片、视频、地点、评论、新闻、学术文献、购物结果和专利，也可以通过图片搜索、将网页抓取为 Markdown 文件，以及查询账户剩余 Credits 和 QPS。

## 下载

| 平台 | 下载 | SHA-256 |
| --- | --- | --- |
| Linux x86_64 | [linux-amd64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_amd64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_amd64.tar.gz.sha256) |
| Linux arm64 | [linux-arm64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_arm64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_arm64.tar.gz.sha256) |
| Windows x86_64 | [windows-amd64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_amd64.zip) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_amd64.zip.sha256) |
| Windows arm64 | [windows-arm64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_arm64.zip) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_arm64.zip.sha256) |
| macOS x86_64 | [darwin-amd64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_amd64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_amd64.tar.gz.sha256) |
| macOS arm64 | [darwin-arm64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_arm64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_arm64.tar.gz.sha256) |

## 使用方法

### API Key

解压后，将 `serper` 放入 `PATH`，或在解压目录中直接运行。Windows 下的可执行文件为 `serper.exe`。发起请求前，将 `SERPER_KEY` 设为你的 Serper API Key。

```bash
export SERPER_KEY="<你的-serper-api-key>"
serper --help
```

PowerShell 中使用 `$env:SERPER_KEY = "<你的-serper-api-key>"`。

### 查询余额和速率限制

```bash
serper credit
```

```text
Timestamp      : 2026-11-17 10:00:00
Serper Credits : 1652
Serper QPS     : 5
```

命令从账户接口读取 `balance` 和 `rateLimit`，分别显示剩余 Credits 和 QPS。时间戳是完整收到响应时的本机系统时间。错误写入标准错误，退出状态为非零；可用 `--timeout` 指定请求超时秒数。

### 搜索

```bash
serper aggregated --query "AI news" --search-num 20 --country US --language en --search-time day
serper general --query "AI news" --search-num 10 --search-time week
serper image --query "serper logo" --search-num 10
serper video --query "web scraping tutorial"
serper place --query "coffee" --location "Berlin,Germany"
serper maps --query "coffee" --ll "@52.5200,13.4050,14z" --search-num 20
serper reviews --cid "<google-cid>" --sort-by newest
serper news --query "OpenAI" --search-time day
serper lens --image-url "https://example.com/image.jpg"
serper scholar --query "large language models"
serper shopping --query "mechanical keyboard"
serper patents --query "battery management"
```

搜索命令输出紧凑的单行 JSON。成功响应包含 `success`、`meta`、`data` 和 `credits`；请求失败时，标准输出会返回 JSON 错误，退出状态为非零。`aggregated` 会合并网页、新闻和图片结果。

`--search-num` 接受 `1`–`100`。`aggregated` 默认请求 `20` 条，其余搜索命令默认请求 `10` 条。非图片搜索会向上取整到 10 的倍数并自动翻页；图片搜索一次请求 10 或 100 条。合并分页时会去重。地图查询需要多页结果时，必须提供 `--ll`。

### 抓取网页

```bash
serper scrape --output example --url "https://www.example.com"
serper scrape --output example --path ./exports --url "https://www.example.com" --timeout 120
```

命令会在需要时创建输出目录，请求成功后写入 `<output>.md`。成功时输出 `true`；失败时输出 `false` 和错误原因，退出状态为非零。已有文件和符号链接不会被覆盖。默认请求 Markdown；可用 `--include-markdown=false` 关闭。

```markdown
## title: ...
## description: ...
## url: ...
## credits: ...

---

网页内容
```

### 命令和参数参考

| 命令 | 用途 |
| --- | --- |
| `aggregated` | 同时搜索网页、新闻和图片。 |
| `general`、`image`、`video`、`news` | 搜索单一类型的结果。 |
| `place`、`maps`、`reviews` | 搜索地点、地图或评论。 |
| `lens` | 使用公开图片 URL 搜索。 |
| `scholar`、`shopping`、`patents` | 搜索学术文献、购物结果或专利。 |
| `scrape` | 将网页保存为 Markdown。 |
| `credit` | 显示剩余 Credits 和账户 QPS。 |

| 参数 | 适用命令 | 用法 |
| --- | --- | --- |
| `--query TEXT` | 除 `reviews`、`lens` 之外的搜索命令 | 搜索关键词；这些命令必填。 |
| `--search-num N` | 搜索命令 | 请求的结果数量，为 `1`–`100`，按上述规则归一化。 |
| `--country COUNTRY` | 除 `patents` 之外的搜索命令 | 国家名称或 ISO 代码，默认 `US`；无法识别的名称回退到 `US`。 |
| `--language CODE` | 除 `patents` 之外的搜索命令 | 语言代码，例如 `en`。 |
| `--search-time TIME` | `aggregated`、`general`、`image`、`video`、`news` | `hour`、`day`、`week`、`month` 或 `year`。 |
| `--location LOCATION` | `place` | 地点提示。 |
| `--ll COORDINATES` | `maps` | 地图坐标；查询多页结果时必填。 |
| `--place-id ID` | `maps`、`reviews` | Google Place ID。 |
| `--cid CID` | `maps`、`reviews` | Google CID。 |
| `--fid FID` | `reviews` | Google FID。 |
| `--sort-by ORDER` | `reviews` | 评论排序方式，例如 `newest`。 |
| `--image-url URL` | `lens` | 公开图片 URL，必填。 |
| `--output NAME` | `scrape` | 输出文件名，必填；没有 `.md` 后缀时会自动添加。 |
| `--path DIR` | `scrape` | 输出目录，默认当前目录。 |
| `--url URL` | `scrape` | 网页 URL，必填。 |
| `--include-markdown[=true\|false]` | `scrape` | 是否请求 Markdown，默认 `true`。 |
| `--timeout SECONDS` | 所有命令 | 单次请求超时，单位为秒；要求正整数，默认 `120`。 |
| `--help` | 所有命令 | 显示对应命令的帮助。 |

`reviews` 至少需要 `--fid`、`--cid` 或 `--place-id` 中的一项。用 `serper <命令> --help` 查看各命令的参数。参数值可写成 `--选项 值` 或 `--选项=值`。

## 从源码构建

使用 Rust 1.99.1 和目标平台的 C 编译工具链。Rustup 会根据 [rust-toolchain.toml](rust-toolchain.toml) 选择版本。

```bash
git clone https://github.com/Joey-Kot/Serper.git
cd Serper
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
cargo build --release --locked
```

可执行文件位于 `target/release/serper`，Windows 下为 `serper.exe`。要构建 Linux x86_64 静态版本，先安装 `musl-gcc`（Ubuntu 的 `musl-tools` 软件包），再运行：

```bash
rustup target add --toolchain 1.99.1 x86_64-unknown-linux-musl
cargo build --release --locked --target x86_64-unknown-linux-musl
```

可执行文件位于 `target/x86_64-unknown-linux-musl/release/serper`。Linux arm64 使用 `aarch64-unknown-linux-musl`。也可以用 `cargo install --path . --locked --target x86_64-unknown-linux-musl` 将静态版本安装到 Cargo 的可执行文件目录，arm64 请替换为对应目标。

运行项目检查：

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

## 许可证

本项目采用 MIT 许可证。 [许可证全文](LICENSE) · [第三方许可证声明](THIRD_PARTY_NOTICES.txt)
