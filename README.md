# Richard

[![Project Sandbox](https://docs.outscale.com/fr/userguide/_images/Project-Sandbox-yellow.svg)](https://docs.outscale.com/en/userguide/Open-Source-Projects.html) [![](https://dcbadge.limes.pink/api/server/HUVtY5gT6s?style=flat&theme=default-inverted)](https://discord.gg/HUVtY5gT6s)

<p align="center">
  <img alt="Chatbot Icon" src="https://img.icons8.com/?size=80&id=VwKX31ZGdnYL&format=png&color=000000" width="80px">
  <br/>
  <strong>A modular Rust-based chatbot for triggering alerts and integrations.</strong>
</p>

---

## 🌐 Links

* 🔧 Example Configuration: [config.env.ori](./config.env.ori)
* 🛠 Contribution Guide: [CONTRIBUTING.md](./CONTRIBUTING.md)
* 💬 Join us on [Discord](https://discord.gg/YOUR_INVITE_CODE)

---

## 📄 Table of Contents

* [Overview](#-overview)
* [Features](#-features)
* [Requirements](#-requirements)
* [Installation](#-installation)
* [Configuration](#-configuration)
* [Usage](#-usage)
* [License](#-license)

---

## 🧭 Overview

**Richard** is a friendly, modular chatbot that helps trigger alerts and interact with external services.

It is designed to be extensible via standalone modules and currently supports integration with **Webex**, **GitHub**, **RSS feeds**, **Ollama**, and more.

---

## ✨ Features

Richard has a modular design — each feature must be explicitly enabled via configuration.

### Available Modules

| Module                  | Description                                                             |
| ----------------------- | ----------------------------------------------------------------------- |
| `webex`                 | Interface with Webex chat service                                       |
| `ping`                  | Responds to `/ping` with `pong`                                         |
| `help`                  | Responds to `/help` command                                             |
| `triggers`              | Dispatches commands to all enabled modules                              |
| `down_detectors`        | Monitors one or more URLs; alerts if a target becomes unreachable       |
| `github_orgs`           | Watches for new releases across all repos in one or more GitHub orgs    |
| `github_repos`          | Watches specific GitHub repos for new releases                          |
| `hello`                 | Sends random quotes at regular time intervals                           |
| `feeds`                 | Monitors RSS feeds and alerts on new items                              |
| `roll`                  | Responds to `/roll` dice commands (e.g. `/roll 1d20`)                   |
| `webpages`              | Monitors webpages and alerts when content changes                       |
| `outscale_api_versions` | Watches for new Outscale API versions on selected endpoints             |

---

## ✅ Requirements

* [Rust toolchain](https://www.rust-lang.org/tools/install)
* `musl-tools` (optional, for building static binaries)

---

## ⚙ Installation

### Build for your platform

```bash
cargo build --release
```

### Build a static binary

```bash
rustup target add x86_64-unknown-linux-musl
sudo apt install musl-tools         # For Debian/Ubuntu
cargo build --target x86_64-unknown-linux-musl --release
```

---

## ⚙ Configuration

Richard is configured using environment variables.

1. Copy the sample config:

   ```bash
   cp config.env.ori config.env
   ```
2. Edit the file:

   ```bash
   vim config.env
   ```
3. Load it:

   ```bash
   source config.env
   ```

To see required variables per module:

```bash
./target/release/richard --show-params
```

---

## 🚀 Usage

```bash
source config.env
cargo run
```

---

## 📜 License

**Richard** is licensed under the BSD 3-Clause License.
© Outscale SAS
